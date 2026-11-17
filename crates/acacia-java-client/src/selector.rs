//! `EntitySelectorParser` with `EntitySelectorOptions`, kept to what decides how far a selector reads and whether it
//! parses: each option's value reader, which options may repeat, and the result's limit and player-only flags.

use acacia_java_proto::entity_types::ENTITY_TYPES;

use crate::command_reader::{Fail, Parsed, Reader, is_uuid};
use crate::selector_values::{bounds, map, non_negative};
use crate::snbt::skip_compound;

/// What `EntityArgument` and its kin check of a parsed selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Selector {
    pub max_results: i32,
    pub includes_entities: bool,
    pub current_entity: bool,
}

/// `MessageArgument` steps over these two and fails on the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectorError {
    MissingType,
    UnknownType,
    Other,
}

impl From<Fail> for SelectorError {
    fn from(_: Fail) -> Self {
        SelectorError::Other
    }
}

/// `InvertableSetOptionState`: one plain value, or any number of negated ones and tags.
#[derive(Debug, Default)]
struct Invertable {
    single: bool,
    multiple: bool,
    tags: Vec<String>,
}

impl Invertable {
    fn can_parse_any(&self) -> bool {
        !self.single
    }

    fn can_parse(&self, inverted: bool) -> bool {
        if inverted { !self.single } else { !self.single && !self.multiple }
    }

    fn mark(&mut self, inverted: bool) {
        if inverted { self.multiple = true } else { self.single = true }
    }
}

#[derive(Debug, Default)]
struct Options {
    name: Invertable,
    gamemode: Invertable,
    team: Invertable,
    kind: Invertable,
    /// Options that may appear once: distance, level, x, y, z, dx, dy, dz, the rotations, limit, sort, scores,
    /// advancements.
    once: Vec<&'static str>,
}

struct Parser<'r, 'a> {
    r: &'r mut Reader<'a>,
    selector: Selector,
    options: Options,
}

/// `EntitySelectorParser.parse`.
pub(crate) fn parse_selector(r: &mut Reader, allow_selectors: bool) -> Result<Selector, SelectorError> {
    let selector = Selector { max_results: 0, includes_entities: false, current_entity: false };
    let mut p = Parser { r, selector, options: Options::default() };
    if p.r.at('@') {
        if !allow_selectors {
            return Err(SelectorError::Other);
        }
        p.r.skip();
        p.selector_type()?;
    } else {
        p.name_or_uuid()?;
    }
    Ok(p.selector)
}

impl Parser<'_, '_> {
    fn selector_type(&mut self) -> Result<(), SelectorError> {
        if !self.r.can_read() {
            return Err(SelectorError::MissingType);
        }
        let start = self.r.cursor;
        let (max_results, includes_entities, current_entity) = match char::from_u32(u32::from(self.r.read())) {
            Some('a') => (i32::MAX, false, false),
            Some('e') => (i32::MAX, true, false),
            Some('n') => (1, true, false),
            Some('p' | 'r') => (1, false, false),
            Some('s') => (1, true, true),
            _ => {
                self.r.cursor = start;
                return Err(SelectorError::UnknownType);
            }
        };
        self.selector = Selector { max_results, includes_entities, current_entity };
        if self.r.at('[') {
            self.r.skip();
            self.options()?;
        }
        Ok(())
    }

    fn name_or_uuid(&mut self) -> Parsed {
        let name = self.r.read_string()?;
        if is_uuid(&name) {
            self.selector.includes_entities = true;
        } else if name.is_empty() || name.encode_utf16().count() > 16 {
            return Err(Fail);
        }
        self.selector.max_results = 1;
        Ok(())
    }

    fn options(&mut self) -> Parsed {
        self.r.skip_whitespace();
        while self.r.can_read() && !self.r.at(']') {
            self.r.skip_whitespace();
            let key = self.r.read_string()?;
            self.r.skip_whitespace();
            if !self.r.at('=') {
                return Err(Fail);
            }
            self.r.skip();
            self.r.skip_whitespace();
            self.option(&key)?;
            self.r.skip_whitespace();
            if self.r.can_read() {
                if !self.r.at(',') {
                    if !self.r.at(']') {
                        return Err(Fail);
                    }
                    break;
                }
                self.r.skip();
            }
        }
        self.r.expect(']')
    }

    /// `shouldInvertValue` / `isTag`: an optional prefix character with whitespace around it.
    fn prefix(&mut self, c: char) -> bool {
        self.r.skip_whitespace();
        let found = self.r.at(c);
        if found {
            self.r.skip();
            self.r.skip_whitespace();
        }
        found
    }

    /// An option allowed once (and `limit`/`sort` not on `@s`).
    fn once(&mut self, key: &'static str) -> Parsed {
        let blocked = (key == "limit" || key == "sort") && self.selector.current_entity;
        if blocked || self.options.once.contains(&key) {
            return Err(Fail);
        }
        self.options.once.push(key);
        Ok(())
    }

    fn invertable(&mut self, key: &str) -> &mut Invertable {
        match key {
            "name" => &mut self.options.name,
            "team" => &mut self.options.team,
            _ => &mut self.options.gamemode,
        }
    }

    fn option(&mut self, key: &str) -> Parsed {
        const ONCE: [&str; 14] =
            ["distance", "level", "x", "y", "z", "dx", "dy", "dz", "x_rotation", "y_rotation", "limit", "sort", "scores", "advancements"];
        if let Some(&once) = ONCE.iter().find(|k| **k == key) {
            self.once(once)?;
        }
        match key {
            "name" | "team" | "gamemode" => {
                if !self.invertable(key).can_parse_any() {
                    return Err(Fail);
                }
                let inverted = self.prefix('!');
                let value = if key == "name" { self.r.read_string()? } else { self.r.read_unquoted() };
                let known_mode = ["survival", "creative", "adventure", "spectator"].contains(&value.as_str());
                if !self.invertable(key).can_parse(inverted) || (key == "gamemode" && !known_mode) {
                    return Err(Fail);
                }
                self.invertable(key).mark(inverted);
                if key == "gamemode" {
                    self.selector.includes_entities = false;
                }
            }
            "distance" => non_negative(bounds::<f64>(self.r, true)?)?,
            "level" => {
                non_negative(bounds::<i32>(self.r, true)?)?;
                self.selector.includes_entities = false;
            }
            "x" | "y" | "z" | "dx" | "dy" | "dz" => {
                self.r.read_number::<f64>()?;
            }
            "x_rotation" | "y_rotation" => {
                bounds::<f32>(self.r, false)?;
            }
            "limit" => {
                let count = self.r.read_number::<i32>()?;
                if count < 1 {
                    return Err(Fail);
                }
                self.selector.max_results = count;
            }
            "sort" => {
                let order = self.r.read_unquoted();
                if !["nearest", "furthest", "random", "arbitrary"].contains(&order.as_str()) {
                    return Err(Fail);
                }
            }
            "type" => self.entity_type()?,
            "tag" => {
                self.prefix('!');
                self.r.read_unquoted();
            }
            "nbt" => {
                self.prefix('!');
                skip_compound(self.r)?;
            }
            "scores" | "advancements" => {
                if map(self.r, key == "advancements")? && key == "advancements" {
                    self.selector.includes_entities = false;
                }
            }
            "predicate" => {
                self.prefix('!');
                self.r.read_identifier()?;
            }
            _ => return Err(Fail),
        }
        Ok(())
    }

    fn entity_type(&mut self) -> Parsed {
        if !self.options.kind.can_parse_any() {
            return Err(Fail);
        }
        let inverted = self.prefix('!');
        if self.prefix('#') {
            let id = self.r.read_identifier()?;
            let state = &mut self.options.kind;
            if state.single || state.tags.contains(&id) {
                return Err(Fail);
            }
            state.multiple = true;
            state.tags.push(id);
            return Ok(());
        }
        if !self.options.kind.can_parse(inverted) {
            return Err(Fail);
        }
        let id = self.r.read_identifier()?;
        let id = if id.contains(':') { id } else { format!("minecraft:{id}") };
        if !ENTITY_TYPES.contains(&id.as_str()) {
            return Err(Fail);
        }
        if id == "minecraft:player" && !inverted {
            self.selector.includes_entities = false;
        }
        self.options.kind.mark(inverted);
        Ok(())
    }
}

#[cfg(test)]
#[path = "selector_tests.rs"]
mod tests;
