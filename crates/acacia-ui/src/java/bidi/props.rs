//! The Unicode data ICU4J's `UBiDiProps` answers from (`Bidi_Class`, `Bidi_Mirroring_Glyph`,
//! `Bidi_Paired_Bracket_Type`), read from `icu_properties` (both are built from ICU 78's data, Unicode 17).

use icu_properties::CodePointMapData;
use icu_properties::props::{BidiClass, BidiMirroringGlyph, BidiPairedBracketType};

pub(super) const L: u8 = 0;
pub(super) const R: u8 = 1;
pub(super) const EN: u8 = 2;
pub(super) const ES: u8 = 3;
pub(super) const ET: u8 = 4;
pub(super) const AN: u8 = 5;
pub(super) const CS: u8 = 6;
pub(super) const B: u8 = 7;
pub(super) const S: u8 = 8;
pub(super) const WS: u8 = 9;
pub(super) const ON: u8 = 10;
pub(super) const LRE: u8 = 11;
pub(super) const LRO: u8 = 12;
pub(super) const AL: u8 = 13;
pub(super) const RLE: u8 = 14;
pub(super) const RLO: u8 = 15;
pub(super) const PDF: u8 = 16;
pub(super) const NSM: u8 = 17;
pub(super) const BN: u8 = 18;
pub(super) const FSI: u8 = 19;
pub(super) const LRI: u8 = 20;
pub(super) const RLI: u8 = 21;
pub(super) const PDI: u8 = 22;
pub(super) const ENL: u8 = 23;
pub(super) const ENR: u8 = 24;

/// `UBiDiProps.getClass` (`UCharacterDirection` numbering, which `BidiClass` shares).
pub fn class(c: u32) -> u8 {
    let class = CodePointMapData::<BidiClass>::new().get32(c);
    CLASSES.iter().position(|&k| k == class).map_or(ON, |i| i as u8)
}

/// `BidiClass` values in `UCharacterDirection` order.
const CLASSES: [BidiClass; 23] = [
    BidiClass::LeftToRight,
    BidiClass::RightToLeft,
    BidiClass::EuropeanNumber,
    BidiClass::EuropeanSeparator,
    BidiClass::EuropeanTerminator,
    BidiClass::ArabicNumber,
    BidiClass::CommonSeparator,
    BidiClass::ParagraphSeparator,
    BidiClass::SegmentSeparator,
    BidiClass::WhiteSpace,
    BidiClass::OtherNeutral,
    BidiClass::LeftToRightEmbedding,
    BidiClass::LeftToRightOverride,
    BidiClass::ArabicLetter,
    BidiClass::RightToLeftEmbedding,
    BidiClass::RightToLeftOverride,
    BidiClass::PopDirectionalFormat,
    BidiClass::NonspacingMark,
    BidiClass::BoundaryNeutral,
    BidiClass::FirstStrongIsolate,
    BidiClass::LeftToRightIsolate,
    BidiClass::RightToLeftIsolate,
    BidiClass::PopDirectionalIsolate,
];

/// `UCharacter.getMirror`: the mirroring glyph, or `c` itself.
pub fn mirror(c: u32) -> u32 {
    CodePointMapData::<BidiMirroringGlyph>::new().get32(c).mirroring_glyph.map_or(c, u32::from)
}

/// `Bidi_Paired_Bracket_Type` as ICU numbers it (0 none, 1 open, 2 close).
pub fn bracket_type(c: u32) -> u8 {
    match CodePointMapData::<BidiMirroringGlyph>::new().get32(c).paired_bracket_type {
        BidiPairedBracketType::Open => 1,
        BidiPairedBracketType::Close => 2,
        _ => 0,
    }
}

/// `UCharacter.getBidiPairedBracket(c)` when `c` is an opening bracket (`Bidi_Paired_Bracket_Type=Open`).
pub(super) fn closing_bracket(c: u16) -> Option<u16> {
    let g = CodePointMapData::<BidiMirroringGlyph>::new().get32(u32::from(c));
    if g.paired_bracket_type != BidiPairedBracketType::Open {
        return None;
    }
    g.mirroring_glyph.map(|m| m as u32 as u16).filter(|&m| m != c)
}

pub(super) const fn flag(d: u8) -> u32 {
    1 << d
}

pub(super) const MASK_LTR: u32 = flag(L) | flag(EN) | flag(ENL) | flag(ENR) | flag(AN) | flag(LRE) | flag(LRO) | flag(LRI);
pub(super) const MASK_RTL: u32 = flag(R) | flag(AL) | flag(RLE) | flag(RLO) | flag(RLI);
pub(super) const MASK_EXPLICIT: u32 = flag(LRE) | flag(LRO) | flag(RLE) | flag(RLO) | flag(PDF);
pub(super) const MASK_BN_EXPLICIT: u32 = flag(BN) | MASK_EXPLICIT;
pub(super) const MASK_ISO: u32 = flag(LRI) | flag(RLI) | flag(FSI) | flag(PDI);
pub(super) const MASK_B_S: u32 = flag(B) | flag(S);
pub(super) const MASK_WS: u32 = MASK_B_S | flag(WS) | MASK_BN_EXPLICIT | MASK_ISO;
pub(super) const MASK_POSSIBLE_N: u32 = flag(ON) | flag(CS) | flag(ES) | flag(ET) | MASK_WS;
pub(super) const MASK_EMBEDDING: u32 = flag(NSM) | MASK_POSSIBLE_N;
/// `DirPropFlagMultiRuns`.
pub(super) const MULTI_RUNS: u32 = 1 << 31;

/// `DirPropFlagLR` / `DirPropFlagE` / `DirPropFlagO` by a level's parity.
pub(super) fn flag_lr(level: i8) -> u32 {
    if level & 1 == 0 { flag(L) } else { flag(R) }
}

pub(super) fn flag_e(level: i8) -> u32 {
    if level & 1 == 0 { flag(LRE) } else { flag(RLE) }
}

pub(super) fn flag_o(level: i8) -> u32 {
    if level & 1 == 0 { flag(LRO) } else { flag(RLO) }
}

pub(super) fn no_override(level: i8) -> i8 {
    level & 127
}

/// `GetLRFromLevel`: L or R.
pub(super) fn lr(level: i8) -> u8 {
    (level & 1) as u8
}
