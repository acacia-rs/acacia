use crate::wire::*;
use crate::{Error, List, MAX_DEPTH, Nbt, Result, Str, Value};

/// Reads a root tag: type byte, then (unless `End`) name and payload.
pub fn read<F: Flavor>(r: &mut &[u8]) -> Result<Nbt> {
    read_lossy::<F>(r).map(|(nbt, _)| nbt)
}

/// Like [`read`]; the flag says a string was not UTF-8 and had its bad bytes replaced.
pub fn read_lossy<F: Flavor>(r: &mut &[u8]) -> Result<(Nbt, bool)> {
    let tag = read_u8(r)?;
    if tag == 0 {
        return Ok((Nbt::default(), false));
    }
    let mut lossy = false;
    let name = read_str::<F>(r, &mut lossy)?.into();
    let value = read_payload::<F>(r, tag, 0, &mut lossy)?;
    Ok((Nbt { name, value }, lossy))
}

fn read_str<F: Flavor>(r: &mut &[u8], lossy: &mut bool) -> Result<Str> {
    let n = F::read_str_len(r)?;
    let b = take(r, n)?;
    Ok(match std::str::from_utf8(b) {
        Ok(s) => Str::new(s),
        Err(_) => {
            *lossy = true;
            Str::from_utf8_lossy(b)
        }
    })
}

/// Initial capacity for a decoded count, bounded by the remaining input so a hostile count can't OOM.
#[inline]
fn cap(n: usize, r: &[u8]) -> usize {
    n.min(r.len())
}

fn read_payload<F: Flavor>(r: &mut &[u8], tag: u8, depth: usize, lossy: &mut bool) -> Result<Value> {
    if depth > MAX_DEPTH {
        return Err(Error::TooDeep(MAX_DEPTH));
    }
    Ok(match tag {
        0 => Value::End,
        1 => Value::Byte(read_i8(r)?),
        2 => Value::Short(read_li16(r)?),
        3 => Value::Int(F::read_int(r)?),
        4 => Value::Long(F::read_long(r)?),
        5 => Value::Float(read_lf32(r)?),
        6 => Value::Double(read_lf64(r)?),
        7 => {
            let n = read_len::<F>(r)?;
            Value::ByteArray(take(r, n)?.to_vec())
        }
        8 => Value::String(read_str::<F>(r, lossy)?),
        9 => {
            let (tag, n) = read_list_header::<F>(r)?;
            let mut items = Vec::with_capacity(cap(n, r));
            for _ in 0..n {
                items.push(read_payload::<F>(r, tag, depth + 1, lossy)?);
            }
            Value::List(List { tag, items })
        }
        10 => {
            let mut entries = Vec::new();
            loop {
                let tag = read_u8(r)?;
                if tag == 0 {
                    break;
                }
                let name = read_str::<F>(r, lossy)?;
                entries.push((name, read_payload::<F>(r, tag, depth + 1, lossy)?));
            }
            Value::Compound(entries)
        }
        11 => {
            let n = read_len::<F>(r)?;
            let mut v = Vec::with_capacity(cap(n, r));
            for _ in 0..n {
                v.push(F::read_int(r)?);
            }
            Value::IntArray(v)
        }
        12 => {
            let n = read_len::<F>(r)?;
            let mut v = Vec::with_capacity(cap(n, r));
            for _ in 0..n {
                v.push(F::read_long(r)?);
            }
            Value::LongArray(v)
        }
        _ => return Err(Error::UnknownTag(tag)),
    })
}
