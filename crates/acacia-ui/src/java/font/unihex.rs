//! `UnihexProvider`: GNU Unifont's `.hex` files from a zip, 16 rows of 8 to 32 bits per glyph,
//! drawn at half size (oversample 2) with half-pixel bold and shadow offsets.

use std::collections::HashMap;
use std::io::Read;

/// A glyph's rows, left-aligned in 32 bits, and the columns drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HexGlyph {
    pub rows: [u32; 16],
    pub left: i32,
    pub right: i32,
}

impl HexGlyph {
    pub fn width(&self) -> i32 {
        self.right - self.left + 1
    }

    /// `getAdvance`: `width / 2 + 1`, in integers.
    pub fn advance(&self) -> f32 {
        (self.width() / 2 + 1) as f32
    }

    /// `unpackBitsToBytes`: `width` × 16 texels, opaque white where a bit is set.
    pub fn rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.width().max(0) as usize * 64);
        for row in self.rows {
            for i in (31 - self.right..=31 - self.left).rev() {
                let set = (0..32).contains(&i) && (row >> i) & 1 != 0;
                out.extend(if set { [255; 4] } else { [0; 4] });
            }
        }
        out
    }
}

/// `size_overrides`: `[from, to]` drawn with these columns.
#[derive(Clone, Copy, Debug)]
pub struct Override {
    pub from: u32,
    pub to: u32,
    pub left: i32,
    pub right: i32,
}

fn digit(b: u8) -> Result<u32, String> {
    match b {
        b'0'..=b'9' => Ok(u32::from(b - b'0')),
        b'A'..=b'F' => Ok(u32::from(b - b'A' + 10)),
        _ => Err(format!("expected hex digit, got {}", b as char)),
    }
}

/// `readFromStream`: `CODE:BITS` lines; any malformed line fails the whole provider. Rows and their
/// bit width per codepoint, later entries replacing earlier ones.
pub fn parse(text: &[u8], out: &mut HashMap<u32, ([u32; 16], u32)>) -> Result<(), String> {
    let mut rest = text;
    loop {
        let colon = rest.iter().position(|&b| b == b':');
        let code = &rest[..colon.unwrap_or(rest.len())];
        if code.is_empty() && colon.is_none() {
            return Ok(());
        }
        let Some(colon) = colon.filter(|_| (4..=6).contains(&code.len())) else {
            return Err("expected 4, 5 or 6 hex digits followed by a colon".into());
        };
        let codepoint = code.iter().try_fold(0, |a, &b| Ok::<_, String>(a << 4 | digit(b)?))?;
        rest = &rest[colon + 1..];
        let end = rest.iter().position(|&b| b == b'\n');
        let data = &rest[..end.unwrap_or(rest.len())];
        rest = &rest[end.map_or(rest.len(), |e| e + 1)..];
        let digits = match data.len() {
            32 => 2,
            64 => 4,
            96 => 6,
            128 => 8,
            _ => return Err("expected hex number describing (8,16,24,32) x 16 bitmap".into()),
        };
        let bits = digits as u32 * 4;
        let mut rows = [0u32; 16];
        for (r, row) in rows.iter_mut().enumerate() {
            let v = data[r * digits..(r + 1) * digits].iter().try_fold(0u32, |a, &b| Ok::<_, String>(a << 4 | digit(b)?))?;
            *row = if bits == 32 { v } else { v << (32 - bits) };
        }
        out.insert(codepoint, (rows, bits));
    }
}

/// `LineData.calculateWidth`: the set columns, or `[0, bitWidth]` for a blank glyph.
fn columns(rows: &[u32; 16], bits: u32) -> (i32, i32) {
    let mask = rows.iter().fold(0, |m, r| m | r);
    if mask == 0 { (0, bits as i32) } else { (mask.leading_zeros() as i32, 31 - mask.trailing_zeros() as i32) }
}

/// `Definition.loadData`: every `.hex` entry of the zip, in the zip's order.
pub fn load(zip: &[u8], overrides: &[Override]) -> Result<HashMap<u32, HexGlyph>, String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).map_err(|e| e.to_string())?;
    let mut bits = HashMap::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.name().ends_with(".hex") {
            let mut text = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut text).map_err(|e| e.to_string())?;
            parse(&text, &mut bits)?;
        }
    }
    let mut glyphs = HashMap::with_capacity(bits.len());
    for o in overrides {
        for c in o.from..=o.to {
            if let Some((rows, _)) = bits.remove(&c) {
                glyphs.insert(c, HexGlyph { rows, left: o.left, right: o.right });
            }
        }
    }
    for (c, (rows, width)) in bits {
        let (left, right) = columns(&rows, width);
        glyphs.insert(c, HexGlyph { rows, left, right });
    }
    Ok(glyphs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_widths_and_blank_glyphs() {
        let mut bits = HashMap::new();
        parse(b"0041:0000000018242442427E424242420000\n0020:00000000000000000000000000000000\n", &mut bits).unwrap();
        let (rows, w) = bits[&0x41];
        assert_eq!((w, rows[4]), (8, 0x18 << 24));
        assert_eq!(columns(&rows, w), (1, 6));
        // Blank: columns 0 to 8, nine texels wide.
        let (rows, w) = bits[&0x20];
        assert_eq!(columns(&rows, w), (0, 8));
        let blank = HexGlyph { rows, left: 0, right: 8 };
        assert_eq!((blank.width(), blank.advance(), blank.rgba().len()), (9, 5.0, 9 * 16 * 4));
        assert!(parse(b"zz:00\n", &mut bits).is_err());
        assert!(parse(b"0041:00000000182424424", &mut bits).is_err());
    }
}
