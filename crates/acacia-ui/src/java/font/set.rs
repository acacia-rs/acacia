//! `FontSet`: one font's providers, those whose filter the font options pass, each codepoint drawn
//! by the first that has it (skipping "fishy" advances for the filtered source), and the glyphs
//! grouped by width for obfuscated text.

use std::collections::HashMap;
use std::sync::Arc;

use super::hashset::IntSet;
use super::provider::{Glyph, GlyphTexture, Provider, special};
use crate::java::mth::{JavaRandom, ceil};

/// The font options (`Options.forceUnicodeFont`, `japaneseGlyphVariants`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FontOptions {
    pub uniform: bool,
    pub japanese: bool,
}

/// `FontOption.Filter`: the options a provider needs set (true) or unset (false).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub uniform: Option<bool>,
    pub jp: Option<bool>,
}

impl Filter {
    pub fn parse(v: &serde_json::Value) -> Result<Filter, String> {
        let Some(map) = v.as_object() else { return if v.is_null() { Ok(Filter::default()) } else { Err("filter is not an object".into()) } };
        let mut f = Filter::default();
        for (k, v) in map {
            let v = v.as_bool().ok_or("filter value is not a boolean")?;
            match k.as_str() {
                "uniform" => f.uniform = Some(v),
                "jp" => f.jp = Some(v),
                _ => return Err(format!("unknown font option {k}")),
            }
        }
        Ok(f)
    }

    pub fn apply(&self, o: FontOptions) -> bool {
        self.uniform.is_none_or(|v| v == o.uniform) && self.jp.is_none_or(|v| v == o.japanese)
    }

    /// `merge`: `other`'s values, overridden by this one's.
    pub fn merge(&self, other: &Filter) -> Filter {
        Filter { uniform: self.uniform.or(other.uniform), jp: self.jp.or(other.jp) }
    }
}

#[derive(Clone)]
pub struct Conditional {
    pub provider: Arc<Provider>,
    pub filter: Filter,
}

pub struct FontSet {
    all: Vec<Conditional>,
    active: Vec<Arc<Provider>>,
    by_width: HashMap<i32, Vec<u32>>,
    pub missing: Glyph,
    pub white: Glyph,
}

/// `hasFishyAdvance`: below 0 or above 32, plain or bold.
fn fishy(g: &Glyph) -> bool {
    let (a, b) = (g.advance(false), g.advance(true));
    !(0.0..=32.0).contains(&a) || !(0.0..=32.0).contains(&b)
}

impl FontSet {
    pub fn new(all: Vec<Conditional>, options: FontOptions) -> FontSet {
        let mut set = FontSet { all, active: Vec::new(), by_width: HashMap::new(), missing: special(GlyphTexture::Missing), white: special(GlyphTexture::White) };
        set.reload(options);
        set
    }

    /// `reload(options)` / `selectProviders`.
    pub fn reload(&mut self, options: FontOptions) {
        let selected: Vec<&Arc<Provider>> = self.all.iter().filter(|c| c.filter.apply(options)).map(|c| &c.provider).collect();
        let mut supported = IntSet::default();
        for p in &selected {
            supported.add_all(p.supported());
        }
        let mut used = vec![false; selected.len()];
        self.by_width.clear();
        for c in supported.iter() {
            let c = c as u32;
            if let Some((i, g)) = selected.iter().enumerate().find_map(|(i, p)| p.glyph(c).map(|g| (i, g))) {
                used[i] = true;
                self.by_width.entry(ceil(g.advance(false))).or_default().push(c);
            }
        }
        self.active = selected.iter().zip(used).filter(|(_, u)| *u).map(|(p, _)| Arc::clone(p)).collect();
    }

    /// `Source(filterFishy).getGlyph(c)`.
    pub fn glyph(&self, c: u32, filter_fishy: bool) -> Glyph {
        let mut first = None;
        for p in &self.active {
            if let Some(g) = p.glyph(c) {
                if !fishy(&g) {
                    return if filter_fishy { g } else { first.unwrap_or(g) };
                }
                first.get_or_insert(g);
            }
        }
        match first {
            Some(g) if !filter_fishy => g,
            _ => self.missing.clone(),
        }
    }

    /// `getRandomGlyph(random, width)`: a glyph as wide (rounded up) as `width`; no draw from
    /// `random` when there is none.
    pub fn random_glyph(&self, random: &mut JavaRandom, width: i32) -> Glyph {
        match self.by_width.get(&width).filter(|l| !l.is_empty()) {
            Some(list) => self.glyph(list[random.next_int_bound(list.len() as i32) as usize], true),
            None => self.missing.clone(),
        }
    }

    /// Codepoints by `ceil(advance)` in Java's order (for the parity probe).
    pub fn by_width(&self) -> &HashMap<i32, Vec<u32>> {
        &self.by_width
    }

    pub fn providers(&self) -> impl Iterator<Item = &Arc<Provider>> {
        self.all.iter().map(|c| &c.provider)
    }
}
