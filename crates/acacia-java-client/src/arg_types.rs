//! Command argument types as `ClientboundCommandsPacket` names them (`ArgumentTypeInfos` registry ids and their
//! `serializeToNetwork` properties), each with the `parse` that decides how much input it takes and whether it fits.
//! Boundary: extents and structural errors are vanilla's, but registry and enum values (items, blocks, particles,
//! slots, criteria, colours) aren't checked against the registries, nor SNBT contents or block properties, so a
//! command that fails only on such a value parses here; a later sibling of the same name isn't merged into the first.

use acacia_java_proto::{Error, Wire, read_varint};

use crate::arg_values::{block_state, item_predicate, item_stack, message, nbt_path, pair, position};
use crate::command_reader::{Fail, Parsed, Reader, identifier_char, is_uuid};
use crate::selector::{Selector, parse_selector};
use crate::selector_values::bounds;
use crate::snbt::{skip_compound, skip_value};

/// What the parse may use of the player's permissions: `commands/entity_selectors` (op level 2+).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Source {
    pub selectors: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ArgType {
    Bool,
    Float(Option<f32>, Option<f32>),
    Double(Option<f64>, Option<f64>),
    Integer(Option<i32>, Option<i32>),
    Long(Option<i64>, Option<i64>),
    Word,
    Phrase,
    Greedy,
    Entity { single: bool, players_only: bool },
    GameProfile,
    ScoreHolder { multiple: bool },
    Position { int: bool },
    Pair { int: bool },
    Angle,
    Block { tags: bool },
    ItemStack,
    ItemPredicate,
    /// `readUnquotedString` (colors, objectives, teams, game modes, anchors, display slots, string enums).
    Unquoted,
    HexColor,
    Snbt,
    Compound,
    NbtPath,
    /// `MessageArgument`, the only `SignedArgument`.
    Message,
    /// Up to a space: criteria and item slots.
    ToSpace,
    Operation,
    Swizzle,
    Identifier,
    /// An identifier, or `#` and a tag's.
    Tagged,
    IntRange,
    FloatRange,
    Time { min: i32 },
    ResourceSelector,
    Particle,
    /// `ResourceOrIdArgument`: an identifier or an inline SNBT value.
    ResourceOrId,
    SlotSource,
    Uuid,
}

/// Reads one optional bound pair: a flags byte, then min and max as present.
fn bounded<T: Wire>(input: &mut &[u8]) -> Result<(Option<T>, Option<T>), Error> {
    let flags = u8::read(input)?;
    let min = if flags & 1 != 0 { Some(T::read(input)?) } else { None };
    let max = if flags & 2 != 0 { Some(T::read(input)?) } else { None };
    Ok((min, max))
}

impl ArgType {
    /// The type for registry id `parser` with its properties read from the packet; `None` for an id 26.3 doesn't
    /// have, whose properties can't be skipped.
    pub(crate) fn read(parser: i32, input: &mut &[u8]) -> Result<Option<Self>, Error> {
        use ArgType::*;
        let flags = |input: &mut &[u8]| u8::read(input);
        Ok(Some(match parser {
            0 => Bool,
            1 => bounded(input).map(|(a, b)| Float(a, b))?,
            2 => bounded(input).map(|(a, b)| Double(a, b))?,
            3 => bounded(input).map(|(a, b)| Integer(a, b))?,
            4 => bounded(input).map(|(a, b)| Long(a, b))?,
            5 => [Word, Phrase, Greedy].get(read_varint(input)? as usize).cloned().ok_or(Error::Invalid("string argument kind"))?,
            6 => flags(input).map(|f| Entity { single: f & 1 != 0, players_only: f & 2 != 0 })?,
            7 => GameProfile,
            8 | 10 => Position { int: parser == 8 },
            9 | 11 | 29 => Pair { int: parser == 9 },
            12 | 13 => Block { tags: parser == 13 },
            14 => ItemStack,
            15 => ItemPredicate,
            16 | 24 | 30 | 33 | 38 | 42 | 49..=51 | 60 => Unquoted,
            17 => HexColor,
            18 | 19 | 22 => Snbt,
            20 => Message,
            21 => Compound,
            23 => NbtPath,
            25 | 34 | 35 => ToSpace,
            26 => Operation,
            27 => Particle,
            28 => Angle,
            31 => flags(input).map(|f| ScoreHolder { multiple: f & 1 != 0 })?,
            32 => Swizzle,
            36 | 41 => Identifier,
            37 | 44 | 45 => {
                if parser != 37 {
                    String::read(input)?;
                }
                Tagged
            }
            39 => IntRange,
            40 => FloatRange,
            43 => Time { min: i32::read(input)? },
            46 | 47 | 48 => {
                String::read(input)?;
                if parser == 48 { ResourceSelector } else { Identifier }
            }
            52..=56 | 58 | 59 => ResourceOrId,
            57 => SlotSource,
            61 => Uuid,
            _ => return Ok(None),
        }))
    }

    /// `ArgumentType.parse` for its extent: the reader ends after the argument on success.
    pub(crate) fn parse(&self, r: &mut Reader, source: Source) -> Parsed {
        use ArgType::*;
        match self {
            Bool => r.read_boolean().map(drop),
            Float(min, max) => within(r, *min, *max),
            Double(min, max) => within(r, *min, *max),
            Integer(min, max) => within(r, *min, *max),
            Long(min, max) => within(r, *min, *max),
            Word | Unquoted => Ok(drop(r.read_unquoted())),
            Phrase => r.read_string().map(drop),
            Greedy => {
                r.cursor = r.len();
                Ok(())
            }
            Entity { single, players_only } => {
                let s = selector(r, source)?;
                let fits = !(s.max_results > 1 && *single) && !(s.includes_entities && *players_only && !s.current_entity);
                if fits { Ok(()) } else { Err(Fail) }
            }
            GameProfile => at_selector(r, source, |s| !s.includes_entities),
            ScoreHolder { multiple } => at_selector(r, source, |s| *multiple || s.max_results <= 1),
            Position { int } => position(r, *int),
            Pair { int } => pair(r, *int),
            Angle => angle(r),
            Block { tags } => block_state(r, *tags),
            ItemStack => item_stack(r),
            ItemPredicate => item_predicate(r),
            HexColor => {
                let color = r.read_unquoted();
                let hex = color.chars().all(|c| c.is_ascii_hexdigit());
                if hex && matches!(color.len(), 3 | 6) { Ok(()) } else { Err(Fail) }
            }
            Snbt => skip_value(r),
            Compound => skip_compound(r),
            NbtPath => nbt_path(r),
            Message => message(r, source.selectors),
            ToSpace => Ok(drop(r.read_word())),
            Operation => {
                let operations = ["=", "+=", "-=", "*=", "/=", "%=", "<", ">", "><"];
                if operations.contains(&r.read_word().as_str()) { Ok(()) } else { Err(Fail) }
            }
            Swizzle => swizzle(r),
            Identifier => r.read_identifier().map(drop),
            Tagged => {
                if r.at('#') {
                    r.skip();
                }
                r.read_identifier().map(drop)
            }
            IntRange => bounds::<i32>(r, true).map(drop),
            FloatRange => bounds::<f64>(r, true).map(drop),
            Time { min } => time(r, *min),
            ResourceSelector => Ok(drop(r.read_while(|c| identifier_char(c) || c == u16::from(b'*') || c == u16::from(b'?')))),
            Particle => {
                r.read_identifier()?;
                if r.at('{') { skip_value(r) } else { Ok(()) }
            }
            ResourceOrId => resource_or_id(r),
            SlotSource => {
                if r.at('{') || r.at('[') || r.at('"') || r.at('\'') { resource_or_id(r) } else { Ok(drop(r.read_word())) }
            }
            Uuid => {
                let start = r.cursor;
                let hex = r.read_while(|c| char::from_u32(u32::from(c)).is_some_and(|c| c == '-' || c.is_ascii_hexdigit()));
                if is_uuid(&hex) {
                    Ok(())
                } else {
                    r.cursor = start;
                    Err(Fail)
                }
            }
        }
    }
}

/// Brigadier's number types: the reader's number, then the declared bounds.
fn within<T: std::str::FromStr + PartialOrd + Copy>(r: &mut Reader, min: Option<T>, max: Option<T>) -> Parsed {
    let start = r.cursor;
    let value = r.read_number::<T>()?;
    if min.is_some_and(|m| value < m) || max.is_some_and(|m| value > m) {
        r.cursor = start;
        return Err(Fail);
    }
    Ok(())
}

fn selector(r: &mut Reader, source: Source) -> Parsed<Selector> {
    parse_selector(r, source.selectors).map_err(|_| Fail)
}

/// `GameProfileArgument` and `ScoreHolderArgument`: a selector meeting `fits`, or a word.
fn at_selector(r: &mut Reader, source: Source, fits: impl Fn(&Selector) -> bool) -> Parsed {
    if !r.at('@') {
        r.read_word();
        return Ok(());
    }
    if fits(&selector(r, source)?) { Ok(()) } else { Err(Fail) }
}

fn angle(r: &mut Reader) -> Parsed {
    if !r.can_read() {
        return Err(Fail);
    }
    if r.at('~') {
        r.skip();
    }
    if r.can_read() && !r.at(' ') && !r.read_number::<f32>()?.is_finite() {
        return Err(Fail);
    }
    Ok(())
}

fn swizzle(r: &mut Reader) -> Parsed {
    let mut seen = Vec::new();
    while r.can_read() && !r.at(' ') {
        let axis = r.read();
        if !"xyz".encode_utf16().any(|c| c == axis) || seen.contains(&axis) {
            return Err(Fail);
        }
        seen.push(axis);
    }
    Ok(())
}

/// `TimeArgument`: a float and a unit of `d`, `s`, `t` or none, rounding to at least `min` ticks.
fn time(r: &mut Reader, min: i32) -> Parsed {
    let value = r.read_number::<f32>()?;
    let factor = match r.read_unquoted().as_str() {
        "d" => 24000.0,
        "s" => 20.0,
        "t" | "" => 1.0,
        _ => return Err(Fail),
    };
    // `Math.round(float)`.
    if ((value * factor) + 0.5).floor() < min as f32 { Err(Fail) } else { Ok(()) }
}

/// `ResourceOrIdArgument`'s grammar: an identifier, else an SNBT value.
fn resource_or_id(r: &mut Reader) -> Parsed {
    let start = r.cursor;
    r.skip_whitespace();
    if r.read_identifier_non_empty().is_ok() {
        return Ok(());
    }
    r.cursor = start;
    skip_value(r)
}
