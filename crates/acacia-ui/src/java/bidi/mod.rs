//! `Language.getVisualOrder` as `ClientLanguage` runs it (`FormattedBidiReorder.reorder(text,
//! defaultRightToLeft = false)`): the text flattened with legacy codes resolved (one style per UTF-16
//! unit, `SubStringSource`), Arabic-shaped, laid into ICU Bidi's visual runs (paragraph level
//! `LEVEL_DEFAULT_LTR`), each run cut where the style changes, and right-to-left runs drawn
//! backwards with mirrored characters (`FormattedCharSequence.backward`).
//! Ported from ICU4J 78.3's Bidi, BidiLine and ArabicShaping (Unicode License v3).
//!
//! Shaping can shrink the text (lam-alef ligatures) while the styles stay indexed by the unshaped
//! text, so styles after a ligature shift by one per ligature, as in Java.

mod arabic;
mod brackets;
mod explicit;
mod implicit;
pub mod props;
mod runs;
mod ubidi;

use super::text::{Style, Text};
use ubidi::Bidi;

/// `Bidi.LEVEL_DEFAULT_LTR`.
const DEFAULT_LTR: i8 = 126;
const REPLACEMENT: u32 = 0xFFFD;

/// A character as the font receives it from a `FormattedCharSequence`: its style, and whether its
/// sink position is 0 (where underlines and strikethroughs start a pixel early). In a right-to-left
/// piece that is the piece's last character drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualChar {
    pub first: bool,
    pub style: Style,
    pub codepoint: u32,
}

/// `Language.getVisualOrder(text)`, or, for `text.forward`, the spans as they are
/// (`FormattedCharSequence.forward` of each: no legacy codes, no reordering).
pub fn visual_order(text: &Text) -> Vec<VisualChar> {
    let mut out = Vec::new();
    if text.forward {
        for span in &text.spans {
            forward(&span.text.encode_utf16().collect::<Vec<_>>(), &span.style, &mut out);
        }
        return out;
    }
    let text = text.resolve_legacy();
    let mut plain = Vec::new();
    let mut styles: Vec<&Style> = Vec::new();
    let mut reorders = false;
    for (c, style) in text.chars() {
        reorders |= may_reorder(c);
        for &unit in c.encode_utf16(&mut [0; 2]).iter() {
            plain.push(unit);
            styles.push(style);
        }
    }
    if !reorders {
        substring(&plain, &styles, 0, plain.len(), false, &mut out);
        return out;
    }
    let bidi = Bidi::new(arabic::shape(&plain), DEFAULT_LTR);
    for run in bidi.visual_runs() {
        substring(&bidi.text, &styles, run.start, run.len, run.odd, &mut out);
    }
    out
}

/// Whether `c` can make Bidi give anything but one left-to-right run, or be shaped. Without such
/// characters every paragraph is level 0 and `directionFromFlags` says LTR.
fn may_reorder(c: char) -> bool {
    let c = u32::from(c);
    (0x0600..=0x06FF).contains(&c) || (0xFE70..=0xFEFF).contains(&c) || matches!(props::class(c), props::R | props::AL | props::AN | props::RLE | props::RLO | props::RLI | props::FSI)
}

/// `SubStringSource.substring(start, length, reverse)`, fed to the sink: pieces split where the
/// style changes, in reverse order when `reverse`.
fn substring(plain: &[u16], styles: &[&Style], start: usize, len: usize, reverse: bool, out: &mut Vec<VisualChar>) {
    if len == 0 {
        return;
    }
    let mut pieces = Vec::new();
    let (mut piece_start, mut style) = (start, styles[start]);
    for i in start + 1..start + len {
        if styles[i] != style {
            pieces.push((piece_start, i, style));
            (piece_start, style) = (i, styles[i]);
        }
    }
    pieces.push((piece_start, start + len, style));
    if reverse {
        for (a, b, style) in pieces.into_iter().rev() {
            backward(&plain[a..b], style, out);
        }
    } else {
        for (a, b, style) in pieces {
            forward(&plain[a..b], style, out);
        }
    }
}

fn is_high(u: u16) -> bool {
    (0xD800..0xDC00).contains(&u)
}

fn is_low(u: u16) -> bool {
    (0xDC00..0xE000).contains(&u)
}

fn pair(high: u16, low: u16) -> u32 {
    0x10000 + ((u32::from(high) - 0xD800) << 10) + (u32::from(low) - 0xDC00)
}

fn unit(u: u16) -> u32 {
    if is_high(u) || is_low(u) { REPLACEMENT } else { u32::from(u) }
}

/// `StringDecomposer.iterate`: positions count UTF-16 units from the piece's start.
fn forward(units: &[u16], style: &Style, out: &mut Vec<VisualChar>) {
    let mut emit = |position: usize, codepoint: u32| out.push(VisualChar { first: position == 0, style: style.clone(), codepoint });
    let mut i = 0;
    while i < units.len() {
        let u = units[i];
        match units.get(i + 1) {
            Some(&low) if is_high(u) && is_low(low) => {
                emit(i, pair(u, low));
                i += 1;
            }
            _ => emit(i, unit(u)),
        }
        i += 1;
    }
}

/// `StringDecomposer.iterateBackwards` through `UCharacter.getMirror`.
fn backward(units: &[u16], style: &Style, out: &mut Vec<VisualChar>) {
    let mut emit = |position: usize, codepoint: u32| out.push(VisualChar { first: position == 0, style: style.clone(), codepoint: props::mirror(codepoint) });
    let mut i = units.len();
    while i > 0 {
        i -= 1;
        let u = units[i];
        if is_low(u) && i > 0 && is_high(units[i - 1]) {
            i -= 1;
            emit(i, pair(units[i], u));
        } else {
            emit(i, unit(u));
        }
    }
}

#[cfg(test)]
mod tests;
