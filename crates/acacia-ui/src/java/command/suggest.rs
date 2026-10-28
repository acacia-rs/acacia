//! Brigadier's `Suggestion`, `SuggestionsBuilder` and `Suggestions` (merged through a
//! `java.util.HashSet`, whose order decides ties of the case-insensitive sort),
//! `SharedSuggestionProvider.suggest`, and `CommandDispatcher.getCompletionSuggestions`.

use std::cmp::Ordering;

use super::parse::{Parse, Range};
use super::tree::{ArgType, NodeKind, Tree};

/// `text` replacing units `[start, end)` of the input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

impl Suggestion {
    /// `Suggestion.apply`.
    pub fn apply(&self, input: &[u16]) -> String {
        let mut out: Vec<u16> = input[..self.start.min(input.len())].to_vec();
        out.extend(self.text.encode_utf16());
        out.extend_from_slice(&input[self.end.min(input.len())..]);
        String::from_utf16_lossy(&out)
    }

    /// `expand(command, range)`: widened to `range` with the input around it.
    fn expand(&self, command: &[u16], range: Range) -> Suggestion {
        if (range.start, range.end) == (self.start, self.end) {
            return self.clone();
        }
        let mut text: Vec<u16> = Vec::new();
        if range.start < self.start {
            text.extend_from_slice(&command[range.start..self.start]);
        }
        text.extend(self.text.encode_utf16());
        if range.end > self.end {
            text.extend_from_slice(&command[self.end..range.end]);
        }
        Suggestion { start: range.start, end: range.end, text: String::from_utf16_lossy(&text) }
    }

    /// `hashCode`: `Objects.hash(range, text, null)`.
    fn hash(&self) -> i32 {
        let range = 31i32.wrapping_add(self.start as i32).wrapping_mul(31).wrapping_add(self.end as i32);
        let text = units(&self.text).iter().fold(0i32, |h, &u| h.wrapping_mul(31).wrapping_add(i32::from(u)));
        (31i32.wrapping_add(range)).wrapping_mul(31).wrapping_add(text).wrapping_mul(31)
    }
}

/// `String.compareToIgnoreCase`, unit by unit.
fn compare_ignore_case(a: &str, b: &str) -> Ordering {
    let fold = |u: u16| -> u32 {
        let c = char::from_u32(u32::from(u)).unwrap_or('\u{FFFD}');
        let upper = c.to_uppercase().next().filter(|_| c.to_uppercase().count() == 1).unwrap_or(c);
        let lower = upper.to_lowercase().next().filter(|_| upper.to_lowercase().count() == 1).unwrap_or(upper);
        u32::from(lower)
    };
    let (a, b) = (units(a), units(b));
    for (&x, &y) in a.iter().zip(&b) {
        if x != y && fold(x) != fold(y) {
            return fold(x).cmp(&fold(y));
        }
    }
    a.len().cmp(&b.len())
}

/// A `java.util.HashSet<Suggestion>` built by `add`: iterated bucket by bucket, each bucket in
/// insertion order.
#[derive(Default)]
struct HashSet {
    items: Vec<(i32, Suggestion)>,
}

impl HashSet {
    fn add(&mut self, s: Suggestion) {
        if !self.items.iter().any(|(_, x)| *x == s) {
            let h = s.hash();
            self.items.push((h ^ ((h as u32) >> 16) as i32, s));
        }
    }

    fn into_list(self) -> Vec<Suggestion> {
        // The table starts at 16 and doubles past three quarters full.
        let mut n = 16usize;
        while self.items.len() > n * 3 / 4 {
            n *= 2;
        }
        let mut items = self.items;
        items.sort_by_key(|(h, _)| (*h as u32 as usize) & (n - 1));
        items.into_iter().map(|(_, s)| s).collect()
    }
}

/// `Suggestions`: the range all of them replace, sorted ignoring case.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Suggestions {
    pub range: Option<Range>,
    pub list: Vec<Suggestion>,
}

impl Suggestions {
    /// `Suggestions.create`.
    pub fn create(command: &[u16], suggestions: &[Suggestion]) -> Suggestions {
        if suggestions.is_empty() {
            return Suggestions::default();
        }
        let start = suggestions.iter().map(|s| s.start).min().unwrap_or(0);
        let end = suggestions.iter().map(|s| s.end).max().unwrap_or(0);
        let range = Range { start, end };
        let mut set = HashSet::default();
        for s in suggestions {
            set.add(s.expand(command, range));
        }
        let mut list = set.into_list();
        list.sort_by(|a, b| compare_ignore_case(&a.text, &b.text));
        Suggestions { range: Some(range), list }
    }

    /// `Suggestions.merge`.
    pub fn merge(command: &[u16], input: Vec<Suggestions>) -> Suggestions {
        if input.len() <= 1 {
            return input.into_iter().next().unwrap_or_default();
        }
        let mut set = HashSet::default();
        for s in input.into_iter().flat_map(|s| s.list) {
            set.add(s);
        }
        Suggestions::create(command, &set.into_list())
    }
}

/// `SuggestionsBuilder` over `input`, completing from `start`.
pub struct Builder<'a> {
    input: &'a [u16],
    start: usize,
    remaining: String,
    pub remaining_lower: String,
    result: Vec<Suggestion>,
}

impl<'a> Builder<'a> {
    pub fn new(input: &'a [u16], start: usize) -> Builder<'a> {
        let remaining = String::from_utf16_lossy(&input[start..]);
        let lower: Vec<u16> = String::from_utf16_lossy(input).to_lowercase().encode_utf16().collect();
        let remaining_lower = String::from_utf16_lossy(lower.get(start..).unwrap_or_default());
        Builder { input, start, remaining, remaining_lower, result: Vec::new() }
    }

    /// `suggest(text)`: nothing when it is what is already there.
    pub fn suggest(&mut self, text: &str) {
        if text != self.remaining {
            self.result.push(Suggestion { start: self.start, end: self.input.len(), text: text.to_owned() });
        }
    }

    pub fn build(self) -> Suggestions {
        Suggestions::create(self.input, &self.result)
    }
}

/// `SharedSuggestionProvider.matchesSubStr`: `pattern` at the start of `input` or after a `.`,
/// `_` or `/`.
pub fn matches_sub_str(pattern: &str, input: &str) -> bool {
    let mut index = 0;
    while !input[index..].starts_with(pattern) {
        match input[index..].find(['.', '_', '/']) {
            Some(i) => index += i + 1,
            None => return false,
        }
    }
    true
}

/// `SharedSuggestionProvider.suggest(values, builder)`.
pub fn suggest_values<'a>(values: impl IntoIterator<Item = &'a str>, mut builder: Builder) -> Suggestions {
    let prefix = builder.remaining.to_lowercase();
    for v in values {
        if matches_sub_str(&prefix, &v.to_lowercase()) {
            builder.suggest(v);
        }
    }
    builder.build()
}

impl Tree {
    /// `listSuggestions` of one node.
    fn node_suggestions(&self, node: usize, builder: Builder) -> Suggestions {
        match &self.nodes[node].kind {
            NodeKind::Literal(literal) => {
                let mut builder = builder;
                if literal.to_lowercase().starts_with(&builder.remaining_lower) {
                    builder.suggest(literal);
                }
                builder.build()
            }
            NodeKind::Argument { suggestions: Some(values), .. } => suggest_values(values.iter().map(String::as_str), builder),
            NodeKind::Argument { ty: ArgType::Bool, .. } => {
                let mut builder = builder;
                for v in ["true", "false"] {
                    if v.starts_with(&builder.remaining_lower) {
                        builder.suggest(v);
                    }
                }
                builder.build()
            }
            _ => Suggestions::default(),
        }
    }

    /// `getCompletionSuggestions(parse, cursor)`.
    pub fn completions(&self, parse: &Parse, cursor: usize) -> Suggestions {
        let (parent, start_pos) = parse.context.suggestion_context(cursor);
        let start = start_pos.min(cursor);
        let truncated = &parse.input[..cursor];
        let all = self.nodes[parent].children.iter().map(|&c| self.node_suggestions(c, Builder::new(truncated, start))).collect();
        Suggestions::merge(&parse.input, all)
    }
}
