//! Streaming Java Edition NBT writer (big-endian, modified-UTF-8 names).
//!
//! Chunk NBT has a fixed shape, so it is written straight into a byte buffer instead of building a tree.
//! The caller keeps nesting balanced: every `compound`/`list_compound` element ends with `end()`.

pub const END: u8 = 0;
pub const BYTE: u8 = 1;
pub const INT: u8 = 3;
pub const LONG: u8 = 4;
pub const BYTE_ARRAY: u8 = 7;
pub const STRING: u8 = 8;
pub const LIST: u8 = 9;
pub const COMPOUND: u8 = 10;
pub const INT_ARRAY: u8 = 11;
pub const LONG_ARRAY: u8 = 12;

pub struct NbtWriter<'a> {
    out: &'a mut Vec<u8>,
}

impl<'a> NbtWriter<'a> {
    /// Starts a root compound named `""`, as region chunks and `level.dat` use.
    pub fn root(out: &'a mut Vec<u8>) -> Self {
        let mut w = Self { out };
        w.tag(COMPOUND, "");
        w
    }

    /// Wraps a buffer without writing a header (for list elements written by hand).
    pub fn raw(out: &'a mut Vec<u8>) -> Self {
        Self { out }
    }

    fn tag(&mut self, id: u8, name: &str) {
        self.out.push(id);
        self.mutf8(name);
    }

    /// Java's modified UTF-8: NUL as `C0 80`, supplementary characters as two 3-byte surrogates.
    fn mutf8(&mut self, s: &str) {
        let len_at = self.out.len();
        self.out.extend_from_slice(&[0, 0]);
        if s.bytes().all(|b| b != 0 && b < 0x80) {
            self.out.extend_from_slice(s.as_bytes());
        } else {
            for c in s.chars() {
                let c = c as u32;
                if c == 0 {
                    self.out.extend_from_slice(&[0xC0, 0x80]);
                } else if c < 0x80 {
                    self.out.push(c as u8);
                } else if c >= 0x10000 {
                    let v = c - 0x10000;
                    self.utf16(0xD800 | (v >> 10));
                    self.utf16(0xDC00 | (v & 0x3FF));
                } else {
                    self.utf16(c);
                }
            }
        }
        let len = (self.out.len() - len_at - 2).min(u16::MAX as usize) as u16;
        self.out[len_at..len_at + 2].copy_from_slice(&len.to_be_bytes());
    }

    fn utf16(&mut self, u: u32) {
        if u < 0x800 {
            self.out
                .extend_from_slice(&[0xC0 | (u >> 6) as u8, 0x80 | (u & 0x3F) as u8]);
        } else {
            self.out.extend_from_slice(&[
                0xE0 | (u >> 12) as u8,
                0x80 | ((u >> 6) & 0x3F) as u8,
                0x80 | (u & 0x3F) as u8,
            ]);
        }
    }

    pub fn byte(&mut self, name: &str, v: i8) {
        self.tag(BYTE, name);
        self.out.push(v as u8);
    }

    pub fn int(&mut self, name: &str, v: i32) {
        self.tag(INT, name);
        self.out.extend_from_slice(&v.to_be_bytes());
    }

    pub fn long(&mut self, name: &str, v: i64) {
        self.tag(LONG, name);
        self.out.extend_from_slice(&v.to_be_bytes());
    }

    pub fn string(&mut self, name: &str, v: &str) {
        self.tag(STRING, name);
        self.mutf8(v);
    }

    pub fn byte_array(&mut self, name: &str, v: &[u8]) {
        self.tag(BYTE_ARRAY, name);
        self.out.extend_from_slice(&(v.len() as i32).to_be_bytes());
        self.out.extend_from_slice(v);
    }

    pub fn int_array(&mut self, name: &str, v: &[i32]) {
        self.tag(INT_ARRAY, name);
        self.out.extend_from_slice(&(v.len() as i32).to_be_bytes());
        for x in v {
            self.out.extend_from_slice(&x.to_be_bytes());
        }
    }

    pub fn long_array(&mut self, name: &str, v: &[u64]) {
        self.tag(LONG_ARRAY, name);
        self.out.extend_from_slice(&(v.len() as i32).to_be_bytes());
        for x in v {
            self.out.extend_from_slice(&x.to_be_bytes());
        }
    }

    pub fn compound(&mut self, name: &str) {
        self.tag(COMPOUND, name);
    }

    /// Opens a list of `len` elements of type `elem`. Compound elements are written as fields then `end()`.
    pub fn list(&mut self, name: &str, elem: u8, len: usize) {
        self.tag(LIST, name);
        self.out.push(if len == 0 { END } else { elem });
        self.out.extend_from_slice(&(len as i32).to_be_bytes());
    }

    /// A bare string as a list element.
    pub fn list_string(&mut self, v: &str) {
        self.mutf8(v);
    }

    pub fn end(&mut self) {
        self.out.push(END);
    }

    /// Copies an already-encoded named tag (e.g. a converted block entity field).
    pub fn raw_bytes(&mut self, bytes: &[u8]) {
        self.out.extend_from_slice(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_with_fields() {
        let mut buf = Vec::new();
        let mut w = NbtWriter::root(&mut buf);
        w.int("x", -2);
        w.string("s", "é");
        w.list("l", STRING, 1);
        w.list_string("a");
        w.end();
        assert_eq!(
            buf,
            [
                10, 0, 0, 3, 0, 1, b'x', 0xFF, 0xFF, 0xFF, 0xFE, 8, 0, 1, b's', 0, 2, 0xC3, 0xA9,
                9, 0, 1, b'l', 8, 0, 0, 0, 1, 0, 1, b'a', 0
            ]
        );
    }

    #[test]
    fn modified_utf8() {
        let mut buf = Vec::new();
        let mut w = NbtWriter::raw(&mut buf);
        w.list_string("\0\u{1F600}");
        assert_eq!(buf, [0, 8, 0xC0, 0x80, 0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
    }
}
