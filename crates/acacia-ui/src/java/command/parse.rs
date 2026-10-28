//! `CommandDispatcher.parse` and the `CommandContextBuilder` it fills: the nodes and arguments
//! read, their ranges, and the errors of the children that failed.

use std::cmp::Ordering;

use super::error::{Error, Kind};
use super::reader::Reader;
use super::tree::{NodeKind, Tree};

/// A `StringRange`, in UTF-16 units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub start: usize,
    pub end: usize,
}

impl Range {
    pub fn at(pos: usize) -> Range {
        Range { start: pos, end: pos }
    }

    fn encompassing(a: Range, b: Range) -> Range {
        Range { start: a.start.min(b.start), end: a.end.max(b.end) }
    }
}

/// A `CommandContextBuilder` without a child (no redirects).
#[derive(Clone, Debug, PartialEq)]
pub struct Context {
    /// Argument ranges by name, in first-read order (`LinkedHashMap`).
    pub arguments: Vec<(String, Range)>,
    pub nodes: Vec<(usize, Range)>,
    pub range: Range,
    pub executable: bool,
}

impl Context {
    fn with_node(&mut self, node: usize, range: Range) {
        self.nodes.push((node, range));
        self.range = Range::encompassing(self.range, range);
    }

    fn with_argument(&mut self, name: &str, range: Range) {
        match self.arguments.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 = range,
            None => self.arguments.push((name.to_owned(), range)),
        }
    }

    /// `findSuggestionContext`: the node whose children complete at `cursor`, and where.
    pub fn suggestion_context(&self, cursor: usize) -> (usize, usize) {
        if self.range.end < cursor {
            return match self.nodes.last() {
                Some(&(node, range)) => (node, range.end + 1),
                None => (0, self.range.start),
            };
        }
        let mut prev = 0;
        for &(node, range) in &self.nodes {
            if range.start <= cursor && cursor <= range.end {
                return (prev, range.start);
            }
            prev = node;
        }
        (prev, self.range.start)
    }
}

/// `ParseResults`: the context, the reader's cursor where parsing stopped, and the errors.
#[derive(Clone, Debug, PartialEq)]
pub struct Parse {
    pub input: Vec<u16>,
    pub context: Context,
    pub cursor: usize,
    /// By node, in insertion order.
    pub errors: Vec<(usize, Error)>,
}

impl Parse {
    pub fn can_read(&self) -> bool {
        self.cursor < self.input.len()
    }

    /// `Commands.getParseException`.
    pub fn parse_error(&self) -> Option<Error> {
        if !self.can_read() {
            return None;
        }
        if let [(_, e)] = self.errors.as_slice() {
            return Some(e.clone());
        }
        let r = Reader { string: &self.input, cursor: self.cursor };
        Some(r.error(if self.context.range.start == self.context.range.end { Kind::UnknownCommand } else { Kind::UnknownArgument }))
    }
}

const SPACE: u16 = b' ' as u16;

impl Tree {
    /// `CommandDispatcher.parse(reader, source)` from `start` (after the slash).
    pub fn parse(&self, input: &[u16], start: usize) -> Parse {
        let context = Context { arguments: Vec::new(), nodes: Vec::new(), range: Range::at(start), executable: false };
        let mut parse = self.parse_nodes(0, Reader { string: input, cursor: start }, context);
        parse.input = input.to_vec();
        parse
    }

    /// `getRelevantNodes`: the literal spelled up to the next space, else every argument.
    fn relevant(&self, node: usize, r: &Reader) -> Vec<usize> {
        let children = &self.nodes[node].children;
        let mut literals = children.iter().filter(|&&c| self.nodes[c].is_literal()).peekable();
        if literals.peek().is_none() {
            return children.clone();
        }
        let end = (r.cursor..r.string.len()).find(|&i| r.string[i] == SPACE).unwrap_or(r.string.len());
        let word = String::from_utf16_lossy(&r.string[r.cursor..end]);
        match literals.find(|&&c| self.nodes[c].name() == word) {
            Some(&c) => vec![c],
            None => children.iter().copied().filter(|&c| !self.nodes[c].is_literal()).collect(),
        }
    }

    /// `child.parse(reader, context)` and the separator check after it.
    fn parse_child(&self, child: usize, r: &mut Reader, context: &mut Context) -> Result<(), Error> {
        let start = r.cursor;
        match &self.nodes[child].kind {
            NodeKind::Literal(literal) => {
                let units: Vec<u16> = literal.encode_utf16().collect();
                let end = start + units.len();
                let fits = r.can_read_n(units.len()) && r.string[start..end] == units[..] && (end == r.string.len() || r.string[end] == SPACE);
                if !fits {
                    return Err(r.error(Kind::LiteralIncorrect(literal.clone())));
                }
                r.cursor = end;
                context.with_node(child, Range { start, end });
            }
            NodeKind::Argument { name, ty, .. } => {
                ty.parse(r)?;
                let range = Range { start, end: r.cursor };
                context.with_argument(name, range);
                context.with_node(child, range);
            }
            NodeKind::Root => {}
        }
        if r.can_read() && r.peek() != SPACE {
            return Err(r.error(Kind::ExpectedSeparator));
        }
        Ok(())
    }

    fn parse_nodes(&self, node: usize, original: Reader, so_far: Context) -> Parse {
        let mut errors: Vec<(usize, Error)> = Vec::new();
        let mut potentials: Vec<Parse> = Vec::new();
        for child in self.relevant(node, &original) {
            let mut context = so_far.clone();
            let mut r = original.clone();
            if let Err(e) = self.parse_child(child, &mut r, &mut context) {
                errors.push((child, e));
                continue;
            }
            context.executable = self.nodes[child].executable;
            if r.can_read_n(2) {
                r.skip();
                potentials.push(self.parse_nodes(child, r, context));
            } else {
                potentials.push(Parse { input: Vec::new(), context, cursor: r.cursor, errors: Vec::new() });
            }
        }
        if potentials.is_empty() {
            return Parse { input: Vec::new(), context: so_far, cursor: original.cursor, errors };
        }
        let end = original.string.len();
        potentials.sort_by(|a, b| {
            let (ar, br) = (a.cursor < end, b.cursor < end);
            match (ar, br) {
                (false, true) => Ordering::Less,
                (true, false) => Ordering::Greater,
                _ => match (a.errors.is_empty(), b.errors.is_empty()) {
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                    _ => Ordering::Equal,
                },
            }
        });
        potentials.swap_remove(0)
    }
}
