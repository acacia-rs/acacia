//! Value readers of selector options: `MinMaxBounds` ranges (also the `int_range`/`float_range` arguments) and the
//! `scores` and `advancements` maps.

use crate::command_reader::{Fail, Parsed, Reader};

fn range_dots(r: &Reader) -> bool {
    r.can_read_n(2) && r.at('.') && r.peek_at(1) == u16::from(b'.')
}

fn bound<T: std::str::FromStr>(r: &mut Reader) -> Parsed<Option<T>> {
    let from = r.cursor;
    while r.can_read() && (matches!(r.peek(), 0x30..=0x39) || r.at('-') || (r.at('.') && !range_dots(r))) {
        r.skip();
    }
    if r.cursor == from {
        return Ok(None);
    }
    r.text(from, r.cursor).parse().map(Some).map_err(|_| Fail)
}

/// `MinMaxBounds.Bounds.fromReader`: `a`, `a..`, `..b` or `a..b`, as `(min, max)`. `Ints` and `Doubles` refuse
/// swapped ends.
pub(crate) fn bounds<T: std::str::FromStr + PartialOrd + Copy>(r: &mut Reader, swapped_fails: bool) -> Parsed<(Option<T>, Option<T>)> {
    let start = r.cursor;
    let read = |r: &mut Reader| -> Parsed<(Option<T>, Option<T>)> {
        if !r.can_read() {
            return Err(Fail);
        }
        let min = bound(r)?;
        let max = if range_dots(r) {
            r.cursor += 2;
            bound(r)?
        } else {
            min
        };
        let swapped = matches!((min, max), (Some(a), Some(b)) if a > b);
        if (min.is_none() && max.is_none()) || (swapped && swapped_fails) { Err(Fail) } else { Ok((min, max)) }
    };
    read(r).inspect_err(|_| r.cursor = start)
}

/// Neither end below zero (`ERROR_RANGE_NEGATIVE`, `ERROR_LEVEL_NEGATIVE`).
pub(crate) fn non_negative<T: PartialOrd + Default>((min, max): (Option<T>, Option<T>)) -> Parsed {
    let negative = |v: &Option<T>| v.as_ref().is_some_and(|v| *v < T::default());
    if negative(&min) || negative(&max) { Err(Fail) } else { Ok(()) }
}

/// The `scores={name=range,...}` and `advancements={id=bool|{criterion=bool,...},...}` maps; true when not empty.
pub(crate) fn map(r: &mut Reader, advancements: bool) -> Parsed<bool> {
    r.expect('{')?;
    r.skip_whitespace();
    let mut any = false;
    while r.can_read() && !r.at('}') {
        r.skip_whitespace();
        if advancements { r.read_identifier().map(drop)? } else { drop(r.read_unquoted()) }
        r.skip_whitespace();
        r.expect('=')?;
        r.skip_whitespace();
        if !advancements {
            bounds::<i32>(r, true)?;
        } else if r.at('{') {
            r.skip_whitespace();
            r.expect('{')?;
            r.skip_whitespace();
            while r.can_read() && !r.at('}') {
                r.skip_whitespace();
                r.read_unquoted();
                r.skip_whitespace();
                r.expect('=')?;
                r.skip_whitespace();
                r.read_boolean()?;
                r.skip_whitespace();
                if r.at(',') {
                    r.skip();
                }
            }
            r.skip_whitespace();
            r.expect('}')?;
            r.skip_whitespace();
        } else {
            r.read_boolean()?;
        }
        any = true;
        r.skip_whitespace();
        if r.at(',') {
            r.skip();
        }
    }
    r.expect('}')?;
    Ok(any)
}
