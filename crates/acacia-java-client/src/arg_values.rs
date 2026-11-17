//! The multi-part argument readers: coordinates (`WorldCoordinates`, `LocalCoordinates`), `BlockStateParser`,
//! `ItemParser`, `ComponentPredicateParser`, `NbtPathArgument` and `MessageArgument`'s text. Registry contents
//! (block, item and component names, property values) aren't checked.

use crate::command_reader::{Fail, Parsed, Reader};
use crate::selector::{SelectorError, parse_selector};
use crate::snbt::{skip_compound, skip_value};

/// `MessageArgument.MAX_LENGTH`.
const MAX_MESSAGE: usize = 256;

fn space(r: &Reader) -> bool {
    r.at(' ')
}

/// `WorldCoordinate.parseInt` / `parseDouble`: `~`, `~n` or `n`.
fn world(r: &mut Reader, int: bool) -> Parsed {
    if r.at('^') || !r.can_read() {
        return Err(Fail);
    }
    let relative = r.at('~');
    if relative {
        r.skip();
    }
    if r.can_read() && !space(r) {
        if int && !relative { r.read_number::<i32>().map(drop)? } else { r.read_number::<f64>().map(drop)? }
    }
    Ok(())
}

/// `LocalCoordinates.readDouble`: `^` or `^n`.
fn local(r: &mut Reader) -> Parsed {
    if !r.at('^') {
        return Err(Fail);
    }
    r.skip();
    if r.can_read() && !space(r) {
        r.read_number::<f64>()?;
    }
    Ok(())
}

/// `count` coordinates read by `one`, a single space apart.
fn coordinates(r: &mut Reader, count: usize, one: impl Fn(&mut Reader) -> Parsed) -> Parsed {
    let start = r.cursor;
    for i in 0..count {
        if i > 0 {
            if !space(r) {
                r.cursor = start;
                return Err(Fail);
            }
            r.skip();
        }
        one(r)?;
    }
    Ok(())
}

/// `BlockPosArgument` and `Vec3Argument`: three local or three world coordinates.
pub(crate) fn position(r: &mut Reader, int: bool) -> Parsed {
    if r.at('^') { coordinates(r, 3, local) } else { coordinates(r, 3, |r| world(r, int)) }
}

/// `ColumnPosArgument`, `Vec2Argument` and `RotationArgument`: two world coordinates.
pub(crate) fn pair(r: &mut Reader, int: bool) -> Parsed {
    if !r.can_read() {
        return Err(Fail);
    }
    coordinates(r, 2, |r| world(r, int))
}

/// `[key=value, ...]` as `readProperties` reads them, values being `readString`s.
fn properties(r: &mut Reader) -> Parsed {
    r.skip();
    r.skip_whitespace();
    let mut keys = Vec::new();
    while r.can_read() && !r.at(']') {
        r.skip_whitespace();
        let key = r.read_string()?;
        if keys.contains(&key) {
            return Err(Fail);
        }
        keys.push(key);
        r.skip_whitespace();
        r.expect('=')?;
        r.skip_whitespace();
        r.read_string()?;
        r.skip_whitespace();
        if r.can_read() {
            if !r.at(',') {
                if !r.at(']') {
                    return Err(Fail);
                }
                break;
            }
            r.skip();
        }
    }
    r.expect(']')
}

/// `BlockStateParser`: a block (or, testing, `#tag`), properties, then an NBT compound.
pub(crate) fn block_state(r: &mut Reader, tags: bool) -> Parsed {
    if r.at('#') {
        if !tags {
            return Err(Fail);
        }
        r.skip();
    }
    r.read_identifier()?;
    if r.at('[') {
        properties(r)?;
    }
    if r.at('{') {
        skip_compound(r)?;
    }
    Ok(())
}

/// `ItemParser`: an item, then `[component=snbt, !removed, ...]`.
pub(crate) fn item_stack(r: &mut Reader) -> Parsed {
    r.read_identifier()?;
    if !r.at('[') {
        return Ok(());
    }
    r.skip();
    let mut seen = Vec::new();
    while r.can_read() && !r.at(']') {
        r.skip_whitespace();
        let removed = r.at('!');
        if removed {
            r.skip();
        }
        let component = r.read_identifier()?;
        if seen.contains(&component) || component.is_empty() {
            return Err(Fail);
        }
        seen.push(component);
        r.skip_whitespace();
        if !removed {
            r.expect('=')?;
            r.skip_whitespace();
            skip_value(r)?;
            r.skip_whitespace();
        }
        if !r.at(',') {
            break;
        }
        r.skip();
        r.skip_whitespace();
        if !r.can_read() {
            return Err(Fail);
        }
    }
    r.expect(']')
}

/// Whitespace, then `c`, as the packrat grammars' terminals take it.
fn token(r: &mut Reader, c: char) -> bool {
    let start = r.cursor;
    r.skip_whitespace();
    if r.at(c) {
        r.skip();
        return true;
    }
    r.cursor = start;
    false
}

fn id(r: &mut Reader) -> Parsed {
    r.skip_whitespace();
    r.read_identifier_non_empty().map(drop)
}

/// `ComponentPredicateParser`: `*`, `#tag` or an item, then `[test|test, !test, ...]` where a test is
/// `component`, `component=snbt` or `predicate~snbt`.
pub(crate) fn item_predicate(r: &mut Reader) -> Parsed {
    if !token(r, '*') {
        token(r, '#');
        id(r)?;
    }
    if !token(r, '[') {
        return Ok(());
    }
    if token(r, ']') {
        return Ok(());
    }
    loop {
        token(r, '!');
        id(r)?;
        if token(r, '=') || token(r, '~') {
            skip_value(r)?;
        }
        if !token(r, '|') && !token(r, ',') {
            break;
        }
    }
    if token(r, ']') { Ok(()) } else { Err(Fail) }
}

/// `NbtPathArgument`: nodes up to a space.
pub(crate) fn nbt_path(r: &mut Reader) -> Parsed {
    let mut first = true;
    while r.can_read() && !space(r) {
        if r.at('"') || r.at('\'') {
            if r.read_string()?.is_empty() {
                return Err(Fail);
            }
            object_node(r)?;
        } else if r.at('[') {
            r.skip();
            if !r.can_read() {
                return Err(Fail);
            }
            if r.at('{') {
                skip_compound(r)?;
            } else if !r.at(']') {
                r.read_number::<i32>()?;
            }
            r.expect(']')?;
        } else if r.at('{') {
            if !first {
                return Err(Fail);
            }
            skip_compound(r)?;
        } else {
            let start = r.cursor;
            r.read_while(|c| !"\"'[].{} ".encode_utf16().any(|x| x == c));
            if r.cursor == start {
                return Err(Fail);
            }
            object_node(r)?;
        }
        first = false;
        if r.can_read() && !space(r) && !r.at('[') && !r.at('{') {
            r.expect('.')?;
        }
    }
    Ok(())
}

fn object_node(r: &mut Reader) -> Parsed {
    if r.at('{') { skip_compound(r) } else { Ok(()) }
}

/// `MessageArgument.Message.parseText`: the rest of the command, whose selectors must parse when allowed.
pub(crate) fn message(r: &mut Reader, allow_selectors: bool) -> Parsed {
    if r.len() - r.cursor > MAX_MESSAGE {
        return Err(Fail);
    }
    if !allow_selectors {
        r.cursor = r.len();
        return Ok(());
    }
    while r.can_read() {
        if !r.at('@') {
            r.skip();
            continue;
        }
        let start = r.cursor;
        match parse_selector(r, true) {
            Ok(_) => {}
            Err(SelectorError::MissingType | SelectorError::UnknownType) => r.cursor = start + 1,
            Err(SelectorError::Other) => return Err(Fail),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "arg_values_tests.rs"]
mod tests;
