//! How much of a command an SNBT value takes, as `SnbtGrammar` reads one for an argument (`parseForCommands`):
//! whitespace before each token, compounds, lists and typed arrays with optional trailing commas, quoted strings,
//! number and unquoted-string tokens, and `name(args)` operations. The structure is followed exactly; what's inside a
//! token (number formats, escape sequences) isn't checked, so a malformed value can pass where vanilla fails.

use crate::command_reader::{Fail, Parsed, Reader};

fn is(r: &Reader, c: char) -> bool {
    r.at(c)
}

/// Whitespace, then `c` (`StringReaderTerms.character`).
fn token(r: &mut Reader, c: char) -> bool {
    let start = r.cursor;
    r.skip_whitespace();
    if is(r, c) {
        r.skip();
        true
    } else {
        r.cursor = start;
        false
    }
}

fn expect(r: &mut Reader, c: char) -> Parsed {
    if token(r, c) { Ok(()) } else { Err(Fail) }
}

/// `repeatedWithTrailingSeparator(entry, ',')` up to `close`.
fn entries(r: &mut Reader, close: char, mut entry: impl FnMut(&mut Reader) -> Parsed) -> Parsed {
    loop {
        if token(r, close) {
            return Ok(());
        }
        entry(r)?;
        if !token(r, ',') {
            return expect(r, close);
        }
    }
}

fn quoted(r: &mut Reader) -> Parsed {
    let quote = r.read();
    while r.can_read() {
        let c = r.read();
        if c == u16::from(b'\\') {
            if !r.can_read() {
                return Err(Fail);
            }
            r.skip();
        } else if c == quote {
            return Ok(());
        }
    }
    Err(Fail)
}

fn key(r: &mut Reader) -> Parsed {
    r.skip_whitespace();
    if is(r, '"') || is(r, '\'') {
        return quoted(r);
    }
    if r.read_unquoted().is_empty() { Err(Fail) } else { Ok(()) }
}

/// One SNBT value.
pub(crate) fn skip_value(r: &mut Reader) -> Parsed {
    r.skip_whitespace();
    if !r.can_read() {
        return Err(Fail);
    }
    if is(r, '"') || is(r, '\'') {
        return quoted(r);
    }
    if token(r, '{') {
        return entries(r, '}', |r| {
            key(r)?;
            expect(r, ':')?;
            skip_value(r)
        });
    }
    if token(r, '[') {
        let start = r.cursor;
        r.skip_whitespace();
        let typed = (is(r, 'B') || is(r, 'I') || is(r, 'L')) && {
            r.skip();
            token(r, ';')
        };
        if !typed {
            r.cursor = start;
        }
        return entries(r, ']', skip_value);
    }
    let starts_number = matches!(r.peek(), 0x30..=0x39) || is(r, '+') || is(r, '-') || is(r, '.');
    if r.read_unquoted().is_empty() {
        return Err(Fail);
    }
    if !starts_number && token(r, '(') {
        return entries(r, ')', skip_value);
    }
    Ok(())
}

/// `TagParser.parseCompoundAsArgument`: a value that must be a compound.
pub(crate) fn skip_compound(r: &mut Reader) -> Parsed {
    let start = r.cursor;
    r.skip_whitespace();
    if !is(r, '{') {
        r.cursor = start;
        return Err(Fail);
    }
    skip_value(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extent(s: &str, f: fn(&mut Reader) -> Parsed) -> Option<usize> {
        let units: Vec<u16> = s.encode_utf16().collect();
        let mut r = Reader::new(&units);
        f(&mut r).ok().map(|()| r.cursor)
    }

    #[test]
    fn values_end_after_their_last_token() {
        assert_eq!(extent(r#"{a: 1b, "b c": [I; 1, 2,], d: {e: 'x\'y'}} rest"#, skip_compound), Some(42));
        assert_eq!(extent("[B, c] x", skip_value), Some(6), "B without ; is a list element");
        assert_eq!(extent("bool(1) x", skip_value), Some(7));
        assert_eq!(extent("-1.5e3f x", skip_value), Some(7));
        assert_eq!(extent("  {} x", skip_compound), Some(4));
        assert_eq!(extent("[1, 2", skip_value), None);
        assert_eq!(extent("abc", skip_compound), None);
    }
}
