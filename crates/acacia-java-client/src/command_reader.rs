//! Brigadier's `StringReader` over a command's UTF-16 units (Java's `char`s, so cursors and ranges match vanilla's),
//! with the readers `Identifier` and `UUID` add.

/// Any `CommandSyntaxException`: only success and the cursor matter here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fail;

pub(crate) type Parsed<T = ()> = Result<T, Fail>;

#[derive(Debug, Clone)]
pub(crate) struct Reader<'a> {
    units: &'a [u16],
    pub cursor: usize,
}

/// `Character.isWhitespace` for a UTF-16 unit.
fn java_whitespace(c: u16) -> bool {
    matches!(c, 0x09..=0x0d | 0x1c..=0x1f)
        || char::from_u32(u32::from(c)).is_some_and(|c| c.is_whitespace() && !matches!(c, '\u{a0}' | '\u{2007}' | '\u{202f}' | '\u{85}'))
}

fn unit(c: char) -> u16 {
    c as u16
}

pub(crate) fn number_char(c: u16) -> bool {
    matches!(c, 0x30..=0x39) || c == unit('.') || c == unit('-')
}

pub(crate) fn unquoted_char(c: u16) -> bool {
    matches!(c, 0x30..=0x39 | 0x41..=0x5a | 0x61..=0x7a) || [unit('_'), unit('-'), unit('.'), unit('+')].contains(&c)
}

/// `Identifier.isAllowedInIdentifier`.
pub(crate) fn identifier_char(c: u16) -> bool {
    matches!(c, 0x30..=0x39 | 0x61..=0x7a) || [unit('_'), unit(':'), unit('/'), unit('.'), unit('-')].contains(&c)
}

impl<'a> Reader<'a> {
    pub(crate) fn new(units: &'a [u16]) -> Self {
        Self { units, cursor: 0 }
    }

    pub(crate) fn len(&self) -> usize {
        self.units.len()
    }

    pub(crate) fn can_read_n(&self, n: usize) -> bool {
        self.cursor + n <= self.units.len()
    }

    pub(crate) fn can_read(&self) -> bool {
        self.can_read_n(1)
    }

    pub(crate) fn peek(&self) -> u16 {
        self.units[self.cursor]
    }

    pub(crate) fn peek_at(&self, offset: usize) -> u16 {
        self.units[self.cursor + offset]
    }

    /// `canRead() && peek() == c`.
    pub(crate) fn at(&self, c: char) -> bool {
        self.can_read() && self.peek() == unit(c)
    }

    pub(crate) fn skip(&mut self) {
        self.cursor += 1;
    }

    pub(crate) fn read(&mut self) -> u16 {
        self.cursor += 1;
        self.units[self.cursor - 1]
    }

    pub(crate) fn text(&self, start: usize, end: usize) -> String {
        String::from_utf16_lossy(&self.units[start..end])
    }

    pub(crate) fn skip_whitespace(&mut self) {
        while self.can_read() && java_whitespace(self.peek()) {
            self.skip();
        }
    }

    /// Reads while `accept` holds; the text read.
    pub(crate) fn read_while(&mut self, accept: impl Fn(u16) -> bool) -> String {
        let start = self.cursor;
        while self.can_read() && accept(self.peek()) {
            self.skip();
        }
        self.text(start, self.cursor)
    }

    /// Up to the next space (`ParserUtils.readWhile(c != ' ')` and its hand-written copies).
    pub(crate) fn read_word(&mut self) -> String {
        self.read_while(|c| c != unit(' '))
    }

    pub(crate) fn expect(&mut self, c: char) -> Parsed {
        if !self.at(c) {
            return Err(Fail);
        }
        self.skip();
        Ok(())
    }

    /// `readInt`, `readLong`, `readFloat`, `readDouble`: the number characters, parsed as `T`.
    pub(crate) fn read_number<T: std::str::FromStr>(&mut self) -> Parsed<T> {
        let start = self.cursor;
        let text = self.read_while(number_char);
        text.parse().map_err(|_| {
            self.cursor = start;
            Fail
        })
    }

    pub(crate) fn read_unquoted(&mut self) -> String {
        self.read_while(unquoted_char)
    }

    /// `readStringUntil` after the opening quote.
    fn read_until(&mut self, terminator: u16) -> Parsed<String> {
        let mut out = Vec::new();
        let mut escaped = false;
        while self.can_read() {
            let c = self.read();
            if escaped {
                if c != terminator && c != unit('\\') {
                    self.cursor -= 1;
                    return Err(Fail);
                }
                out.push(c);
                escaped = false;
            } else if c == unit('\\') {
                escaped = true;
            } else if c == terminator {
                return Ok(String::from_utf16_lossy(&out));
            } else {
                out.push(c);
            }
        }
        Err(Fail)
    }

    pub(crate) fn read_quoted(&mut self) -> Parsed<String> {
        if !self.can_read() {
            return Ok(String::new());
        }
        let quote = self.peek();
        if quote != unit('"') && quote != unit('\'') {
            return Err(Fail);
        }
        self.skip();
        self.read_until(quote)
    }

    pub(crate) fn read_string(&mut self) -> Parsed<String> {
        if self.at('"') || self.at('\'') { self.read_quoted() } else { Ok(self.read_unquoted()) }
    }

    pub(crate) fn read_boolean(&mut self) -> Parsed<bool> {
        let start = self.cursor;
        match self.read_string()?.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => {
                self.cursor = start;
                Err(Fail)
            }
        }
    }

    /// `Identifier.read`: the identifier characters, which must split into a valid namespace and path.
    pub(crate) fn read_identifier(&mut self) -> Parsed<String> {
        let start = self.cursor;
        let raw = self.read_while(identifier_char);
        let (namespace, path) = raw.split_once(':').unwrap_or(("minecraft", &raw));
        if path.contains(':') || namespace == ".." {
            self.cursor = start;
            return Err(Fail);
        }
        Ok(raw)
    }

    /// `Identifier.readNonEmpty`.
    pub(crate) fn read_identifier_non_empty(&mut self) -> Parsed<String> {
        let id = self.read_identifier()?;
        if id.is_empty() { Err(Fail) } else { Ok(id) }
    }
}

/// `UUID.fromString`: five dash-separated hex groups, each fitting a positive `long`, at most 36 characters.
pub(crate) fn is_uuid(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    text.len() <= 36 && parts.len() == 5 && parts.iter().all(|p| !p.is_empty() && i64::from_str_radix(p, 16).is_ok_and(|v| v >= 0) && !p.starts_with('+'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn strings_numbers_and_identifiers_stop_where_brigadier_does() {
        let text = units(r#""a \"b\" c" rest"#);
        let mut r = Reader::new(&text);
        assert_eq!(r.read_string(), Ok(r#"a "b" c"#.to_owned()));
        assert_eq!(r.cursor, 11);
        let text = units("-1.5x");
        let mut r = Reader::new(&text);
        assert_eq!(r.read_number::<f64>(), Ok(-1.5));
        assert_eq!(r.cursor, 4);
        let text = units("1-2");
        assert_eq!(Reader::new(&text).read_number::<i32>(), Err(Fail));
        let text = units("minecraft:stone[x]");
        let mut r = Reader::new(&text);
        assert_eq!(r.read_identifier(), Ok("minecraft:stone".to_owned()));
        let text = units("a:b:c");
        assert_eq!(Reader::new(&text).read_identifier(), Err(Fail));
        assert!(is_uuid("0-0-0-0-0") && !is_uuid("0-0-0-0") && !is_uuid("ffffffffffffffff-0-0-0-0"));
    }
}
