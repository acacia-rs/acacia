//! The server's command tree (`ClientboundCommandsPacket`), kept to find a command's signable arguments the way
//! `SignableCommand.of` does. Parsing is a minimal stand-in for brigadier: argument extents come from the parser's
//! shape (words, bracket depth, quotes), not from the argument types themselves.

use acacia_java_proto::{Error, Wire, read_varint};

const TYPE_MASK: u8 = 3;
const TYPE_LITERAL: u8 = 1;
const TYPE_ARGUMENT: u8 = 2;
const FLAG_REDIRECT: u8 = 8;
const FLAG_CUSTOM_SUGGESTIONS: u8 = 16;

/// `ArgumentTypeInfos` registry ids that need special handling; the rest read as one token.
mod parser {
    pub const FLOAT: i32 = 1;
    pub const DOUBLE: i32 = 2;
    pub const INTEGER: i32 = 3;
    pub const LONG: i32 = 4;
    pub const STRING: i32 = 5;
    pub const ENTITY: i32 = 6;
    pub const BLOCK_POS: i32 = 8;
    pub const COLUMN_POS: i32 = 9;
    pub const VEC3: i32 = 10;
    pub const VEC2: i32 = 11;
    /// `MessageArgument`, the only `SignedArgument`.
    pub const MESSAGE: i32 = 20;
    pub const ROTATION: i32 = 29;
    pub const SCORE_HOLDER: i32 = 31;
    pub const TIME: i32 = 43;
    pub const RESOURCE_OR_TAG: i32 = 44;
    pub const RESOURCE_SELECTOR: i32 = 48;
    pub const LAST: i32 = 61;
}

/// `StringArgumentType.StringType.GREEDY_PHRASE`.
const GREEDY_PHRASE: i32 = 2;

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Root,
    Literal(String),
    Argument { name: String, parser: i32, greedy: bool },
}

#[derive(Debug, Clone, PartialEq)]
struct Node {
    kind: Kind,
    children: Vec<usize>,
    redirect: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct CommandTree {
    nodes: Vec<Node>,
    root: usize,
}

impl CommandTree {
    /// Reads `ClientboundCommandsPacket`; an argument type this client doesn't know fails it.
    pub(crate) fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let count = read_varint(input)?;
        let nodes: Vec<Node> = (0..count).map(|_| read_node(input)).collect::<Result<_, _>>()?;
        let root = index(read_varint(input)?)?;
        let links = nodes.iter().flat_map(|n| n.children.iter().chain(&n.redirect));
        if root >= nodes.len() || links.into_iter().any(|&i| i >= nodes.len()) {
            return Err(Error::Invalid("command node index"));
        }
        Ok(Self { nodes, root })
    }

    /// The `(name, value)` of each signable argument in `command` (no leading `/`), in order; empty when the
    /// command has none or doesn't parse.
    pub(crate) fn signable(&self, command: &str) -> Vec<(String, String)> {
        let mut found = Vec::new();
        self.walk(self.root, command, 0, &mut found);
        found
    }

    /// Parses `command[at..]` below `node`; true when it parses to the end.
    fn walk(&self, node: usize, command: &str, at: usize, found: &mut Vec<(String, String)>) -> bool {
        if at == command.len() {
            return true;
        }
        let Some(node) = self.nodes.get(node) else { return false };
        let below = node.redirect.and_then(|r| self.nodes.get(r)).unwrap_or(node);
        let rest = &command[at..];
        let literal = below.children.iter().find(|&&c| matches!(&self.nodes[c].kind, Kind::Literal(l) if token_is(rest, l)));
        // Brigadier tries only the literal when one matches.
        let candidates: Vec<usize> = match literal {
            Some(&c) => vec![c],
            None => below.children.iter().copied().filter(|&c| matches!(self.nodes[c].kind, Kind::Argument { .. })).collect(),
        };
        for child in candidates {
            let (len, signed) = match &self.nodes[child].kind {
                Kind::Literal(l) => (l.len(), None),
                Kind::Argument { name, parser: ty, greedy } => match extent(*ty, *greedy, rest) {
                    Some(len) => (len, (*ty == parser::MESSAGE).then(|| (name.clone(), rest[..len].to_owned()))),
                    None => continue,
                },
                Kind::Root => continue,
            };
            let end = at + len;
            let next = if end < command.len() { end + 1 } else { end };
            let mark = found.len();
            found.extend(signed);
            if (end == command.len() || command.as_bytes()[end] == b' ') && self.walk(child, command, next, found) {
                return true;
            }
            found.truncate(mark);
        }
        false
    }
}

fn token_is(rest: &str, literal: &str) -> bool {
    rest.strip_prefix(literal).is_some_and(|after| after.is_empty() || after.starts_with(' '))
}

/// How many bytes of `rest` an argument of `parser` takes.
fn extent(parser: i32, greedy: bool, rest: &str) -> Option<usize> {
    let words = match parser {
        parser::MESSAGE => return Some(rest.len()),
        parser::STRING if greedy => return Some(rest.len()),
        parser::BLOCK_POS | parser::VEC3 => 3,
        parser::COLUMN_POS | parser::VEC2 | parser::ROTATION => 2,
        _ => 1,
    };
    let mut at = 0;
    for i in 0..words {
        if i > 0 {
            at += rest[at..].starts_with(' ').then_some(1)?;
        }
        at += token(&rest[at..])?;
    }
    Some(at)
}

/// One token: up to a space outside brackets and quotes (`@e[a=1, b=2]`, `"a b"`, `{"text":"a b"}`).
fn token(rest: &str) -> Option<usize> {
    let (mut depth, mut quote, mut escaped) = (0i32, None, false);
    for (i, c) in rest.char_indices() {
        match (quote, c) {
            (Some(_), _) if escaped => escaped = false,
            (Some(_), '\\') => escaped = true,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '[' | '{' | '(') => depth += 1,
            (None, ']' | '}' | ')') => depth -= 1,
            (None, ' ') if depth <= 0 => return (i > 0).then_some(i),
            _ => {}
        }
    }
    (!rest.is_empty()).then_some(rest.len())
}

fn index(i: i32) -> Result<usize, Error> {
    usize::try_from(i).map_err(|_| Error::NegativeLength(i))
}

fn read_node(input: &mut &[u8]) -> Result<Node, Error> {
    let flags = u8::read(input)?;
    let children = (0..read_varint(input)?).map(|_| read_varint(input).and_then(index)).collect::<Result<_, _>>()?;
    let redirect = if flags & FLAG_REDIRECT != 0 { Some(index(read_varint(input)?)?) } else { None };
    let kind = match flags & TYPE_MASK {
        TYPE_LITERAL => Kind::Literal(String::read(input)?),
        TYPE_ARGUMENT => {
            let name = String::read(input)?;
            let parser = read_varint(input)?;
            let greedy = read_properties(parser, input)?;
            if flags & FLAG_CUSTOM_SUGGESTIONS != 0 {
                String::read(input)?;
            }
            Kind::Argument { name, parser, greedy }
        }
        _ => Kind::Root,
    };
    Ok(Node { kind, children, redirect })
}

/// Skips an argument's `serializeToNetwork` properties; true for a greedy string.
fn read_properties(parser: i32, input: &mut &[u8]) -> Result<bool, Error> {
    let bounded = |input: &mut &[u8], size: usize| -> Result<(), Error> {
        let flags = u8::read(input)?;
        let count = usize::from(flags & 1) + usize::from(flags >> 1 & 1);
        (0..count * size).try_for_each(|_| u8::read(input).map(|_| ()))
    };
    match parser {
        parser::FLOAT | parser::INTEGER => bounded(input, 4)?,
        parser::DOUBLE | parser::LONG => bounded(input, 8)?,
        parser::STRING => return Ok(read_varint(input)? == GREEDY_PHRASE),
        parser::ENTITY | parser::SCORE_HOLDER => {
            u8::read(input)?;
        }
        parser::TIME => {
            i32::read(input)?;
        }
        parser::RESOURCE_OR_TAG..=parser::RESOURCE_SELECTOR => {
            String::read(input)?;
        }
        0..=parser::LAST => {}
        _ => return Err(Error::Invalid("unknown command argument type")),
    }
    Ok(false)
}

#[cfg(test)]
#[path = "command_tree_tests.rs"]
mod tests;
