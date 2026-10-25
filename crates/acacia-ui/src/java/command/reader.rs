//! Brigadier's `StringReader` over UTF-16 units, as Java strings index (cursor positions are the
//! chat input's).

use super::error::{Error, Kind};

#[derive(Clone, Debug)]
pub struct Reader<'a> {
    pub string: &'a [u16],
    pub cursor: usize,
}

const SPACE: u16 = b' ' as u16;

fn ascii(u: u16) -> Option<u8> {
    u8::try_from(u).ok().filter(u8::is_ascii)
}

pub fn is_allowed_number(u: u16) -> bool {
    ascii(u).is_some_and(|c| c.is_ascii_digit() || c == b'.' || c == b'-')
}

pub fn is_allowed_in_unquoted(u: u16) -> bool {
    ascii(u).is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b'+'))
}

fn is_quote(u: u16) -> bool {
    u == u16::from(b'"') || u == u16::from(b'\'')
}

pub fn text(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

impl<'a> Reader<'a> {
    pub fn new(string: &'a [u16]) -> Reader<'a> {
        Reader { string, cursor: 0 }
    }

    pub fn can_read_n(&self, n: usize) -> bool {
        self.cursor + n <= self.string.len()
    }

    pub fn can_read(&self) -> bool {
        self.can_read_n(1)
    }

    pub fn peek(&self) -> u16 {
        self.string[self.cursor]
    }

    pub fn skip(&mut self) {
        self.cursor += 1;
    }

    pub fn remaining_len(&self) -> usize {
        self.string.len().saturating_sub(self.cursor)
    }

    /// `createWithContext(this, ...)`.
    pub fn error(&self, kind: Kind) -> Error {
        Error { kind, input: Some(self.string.to_vec()), cursor: self.cursor as i32 }
    }

    fn number(&mut self) -> String {
        let start = self.cursor;
        while self.can_read() && is_allowed_number(self.peek()) {
            self.skip();
        }
        text(&self.string[start..self.cursor])
    }

    pub fn read_int(&mut self) -> Result<i32, Error> {
        let start = self.cursor;
        let number = self.number();
        if number.is_empty() {
            return Err(self.error(Kind::ExpectedInt));
        }
        number.parse::<i32>().map_err(|_| {
            self.cursor = start;
            self.error(Kind::InvalidInt(number))
        })
    }

    pub fn read_float(&mut self) -> Result<f32, Error> {
        let start = self.cursor;
        let number = self.number();
        if number.is_empty() {
            return Err(self.error(Kind::ExpectedFloat));
        }
        java_parse_float(&number).ok_or_else(|| {
            self.cursor = start;
            self.error(Kind::InvalidFloat(number))
        })
    }

    pub fn read_unquoted(&mut self) -> String {
        let start = self.cursor;
        while self.can_read() && is_allowed_in_unquoted(self.peek()) {
            self.skip();
        }
        text(&self.string[start..self.cursor])
    }

    /// Up to the next space (acacia's `Token` argument).
    pub fn read_token(&mut self) -> String {
        let start = self.cursor;
        while self.can_read() && self.peek() != SPACE {
            self.skip();
        }
        text(&self.string[start..self.cursor])
    }

    fn read_until(&mut self, terminator: u16) -> Result<String, Error> {
        let mut out: Vec<u16> = Vec::new();
        let mut escaped = false;
        while self.can_read() {
            let c = self.peek();
            self.skip();
            if escaped {
                if c != terminator && c != u16::from(b'\\') {
                    self.cursor -= 1;
                    return Err(self.error(Kind::InvalidEscape(text(&[c]))));
                }
                out.push(c);
                escaped = false;
            } else if c == u16::from(b'\\') {
                escaped = true;
            } else if c == terminator {
                return Ok(text(&out));
            } else {
                out.push(c);
            }
        }
        Err(self.error(Kind::ExpectedEndOfQuote))
    }

    pub fn read_string(&mut self) -> Result<String, Error> {
        if !self.can_read() {
            return Ok(String::new());
        }
        let next = self.peek();
        if is_quote(next) {
            self.skip();
            return self.read_until(next);
        }
        Ok(self.read_unquoted())
    }

    pub fn read_bool(&mut self) -> Result<bool, Error> {
        let start = self.cursor;
        let value = self.read_string()?;
        match value.as_str() {
            "" => Err(self.error(Kind::ExpectedBool)),
            "true" => Ok(true),
            "false" => Ok(false),
            _ => {
                self.cursor = start;
                Err(self.error(Kind::InvalidBool(value)))
            }
        }
    }
}

/// `Float.parseFloat` of `[0-9.-]*`: an optional leading minus, digits with at most one point,
/// at least one digit.
pub fn java_parse_float(s: &str) -> Option<f32> {
    let body = s.strip_prefix('-').unwrap_or(s);
    let valid = !body.is_empty() && !body.contains('-') && body.matches('.').count() <= 1 && body.chars().any(|c| c.is_ascii_digit());
    if valid { s.parse::<f32>().ok() } else { None }
}
