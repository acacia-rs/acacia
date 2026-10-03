use bytes::{BufMut, BytesMut};

use super::{Flavor, MAX_DEPTH, Nbt, Network, read, read_len, write};
use crate::DecodeError;
use crate::codec::*;

/// Advances past a root tag without building values.
pub fn skip<F: Flavor>(r: &mut &[u8]) -> Result<()> {
    let tag = read_u8(r)?;
    if tag == 0 {
        return Ok(());
    }
    let n = F::read_str_len(r)?;
    take(r, n)?;
    skip_payload::<F>(r, tag, 0)
}

fn skip_payload<F: Flavor>(r: &mut &[u8], tag: u8, depth: usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(DecodeError::NbtTooDeep(MAX_DEPTH));
    }
    match tag {
        0 => {}
        1 => drop(take(r, 1)?),
        2 => drop(take(r, 2)?),
        3 => drop(F::read_int(r)?),
        4 => drop(F::read_long(r)?),
        5 => drop(take(r, 4)?),
        6 => drop(take(r, 8)?),
        7 => {
            let n = read_len::<F>(r)?;
            take(r, n)?;
        }
        8 => {
            let n = F::read_str_len(r)?;
            take(r, n)?;
        }
        9 => {
            let tag = read_u8(r)?;
            for _ in 0..read_len::<F>(r)? {
                skip_payload::<F>(r, tag, depth + 1)?;
            }
        }
        10 => loop {
            let tag = read_u8(r)?;
            if tag == 0 {
                break;
            }
            let n = F::read_str_len(r)?;
            take(r, n)?;
            skip_payload::<F>(r, tag, depth + 1)?;
        },
        11 => {
            for _ in 0..read_len::<F>(r)? {
                F::read_int(r)?;
            }
        }
        12 => {
            for _ in 0..read_len::<F>(r)? {
                F::read_long(r)?;
            }
        }
        _ => return Err(DecodeError::Nbt("unknown tag type")),
    }
    Ok(())
}

/// Reads root tags until a `0` terminator byte or end of input (minecraft-data `nbtLoop`).
pub fn read_loop(r: &mut &[u8]) -> Result<Vec<Nbt>> {
    let mut out = Vec::new();
    while let Some(&b) = r.first() {
        if b == 0 {
            *r = &r[1..];
            break;
        }
        out.push(read::<Network>(r)?);
    }
    Ok(out)
}

pub fn write_loop(w: &mut BytesMut, v: &[Nbt]) {
    v.iter().for_each(|n| write::<Network>(w, n));
    w.put_u8(0);
}
