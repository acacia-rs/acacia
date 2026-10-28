use crate::wire::*;
use crate::{Error, MAX_DEPTH, Result};

/// Advances past a root tag without building values; accepts exactly what [`crate::read`] accepts.
pub fn skip<F: Flavor>(r: &mut &[u8]) -> Result<()> {
    skip_root::<F, false>(r, &mut false)
}

/// [`skip`] that also validates strings: `true` when [`crate::read_lossy`] would report replaced bytes.
pub(crate) fn skip_checking_utf8<F: Flavor>(r: &mut &[u8]) -> Result<bool> {
    let mut lossy = false;
    skip_root::<F, true>(r, &mut lossy)?;
    Ok(lossy)
}

/// The string under `key` in a root compound, as [`crate::Value::get`] on the decoded document
/// would find it (first match, invalid UTF-8 replaced), with every other entry skipped unbuilt.
pub fn root_string<'a, F: Flavor>(mut doc: &'a [u8], key: &str) -> Result<Option<std::borrow::Cow<'a, str>>> {
    // A key whose bytes appear nowhere is absent; walking the tags would only prove it.
    if !key.is_empty() && memchr::memmem::find(doc, key.as_bytes()).is_none() {
        return Ok(None);
    }
    let r = &mut doc;
    if read_u8(r)? != 10 {
        return Ok(None);
    }
    skip_str::<F, false>(r, &mut false)?;
    loop {
        let tag = read_u8(r)?;
        if tag == 0 {
            return Ok(None);
        }
        let n = F::read_str_len(r)?;
        let name = take(r, n)?;
        if name == key.as_bytes() {
            if tag != 8 {
                return Ok(None);
            }
            let n = F::read_str_len(r)?;
            return Ok(Some(String::from_utf8_lossy(take(r, n)?)));
        }
        skip_payload::<F, false>(r, tag, 1, &mut false)?;
    }
}

fn skip_root<F: Flavor, const UTF8: bool>(r: &mut &[u8], lossy: &mut bool) -> Result<()> {
    let tag = read_u8(r)?;
    if tag == 0 {
        return Ok(());
    }
    skip_str::<F, UTF8>(r, lossy)?;
    skip_payload::<F, UTF8>(r, tag, 0, lossy)
}

#[inline]
fn skip_str<F: Flavor, const UTF8: bool>(r: &mut &[u8], lossy: &mut bool) -> Result<()> {
    let n = F::read_str_len(r)?;
    let b = take(r, n)?;
    // Nearly every key is short ASCII, where the inlined check beats a `from_utf8` call.
    if UTF8 && !b.is_ascii() && std::str::from_utf8(b).is_err() {
        *lossy = true;
    }
    Ok(())
}

fn skip_payload<F: Flavor, const UTF8: bool>(r: &mut &[u8], tag: u8, depth: usize, lossy: &mut bool) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(Error::TooDeep(MAX_DEPTH));
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
        8 => skip_str::<F, UTF8>(r, lossy)?,
        9 => {
            let (tag, n) = read_list_header::<F>(r)?;
            for _ in 0..n {
                skip_payload::<F, UTF8>(r, tag, depth + 1, lossy)?;
            }
        }
        10 => loop {
            let tag = read_u8(r)?;
            if tag == 0 {
                break;
            }
            skip_str::<F, UTF8>(r, lossy)?;
            skip_payload::<F, UTF8>(r, tag, depth + 1, lossy)?;
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
        _ => return Err(Error::UnknownTag(tag)),
    }
    Ok(())
}
