//! The server's command tree (`ClientboundCommandsPacket` resolved as `NodeResolver` builds it), parsed by
//! `brigadier.rs` to find a command's signable arguments.

use acacia_java_proto::{Error, Wire, read_varint};

use crate::arg_types::ArgType;

const TYPE_MASK: u8 = 3;
const TYPE_LITERAL: u8 = 1;
const TYPE_ARGUMENT: u8 = 2;
const FLAG_EXECUTABLE: u8 = 4;
const FLAG_REDIRECT: u8 = 8;
const FLAG_CUSTOM_SUGGESTIONS: u8 = 16;
const FLAG_RESTRICTED: u8 = 32;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Kind {
    /// The root, or a node whose argument type 26.3 doesn't know: `readNode` leaves its stub null, which builds a
    /// `RootCommandNode` that no parent adopts.
    Root,
    Literal(String),
    Argument { name: String, ty: ArgType },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Node {
    pub kind: Kind,
    pub children: Vec<usize>,
    pub redirect: Option<usize>,
    /// Has a command (`executes`).
    pub executable: bool,
    /// Requires `RESTRICTED_COMMAND`.
    pub restricted: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct CommandTree {
    pub nodes: Vec<Node>,
    pub root: usize,
}

impl CommandTree {
    pub(crate) fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let count = read_varint(input)?;
        let mut nodes: Vec<Node> = (0..count).map(|_| read_node(input)).collect::<Result<_, _>>()?;
        let root = index(read_varint(input)?)?;
        let links = nodes.iter().flat_map(|n| n.children.iter().chain(&n.redirect));
        if root >= nodes.len() || links.into_iter().any(|&i| i >= nodes.len()) {
            return Err(Error::Invalid("command node index"));
        }
        let adoptable: Vec<bool> = nodes.iter().map(|n| n.kind != Kind::Root).collect();
        for node in &mut nodes {
            node.children.retain(|&c| adoptable[c]);
        }
        Ok(Self { nodes, root })
    }

    pub(crate) fn node(&self, i: usize) -> &Node {
        &self.nodes[i]
    }

    /// `addChild` keeps the first child of a name (merging a later one into it, which servers don't send).
    pub(crate) fn literal(&self, node: usize, text: &str) -> Option<usize> {
        self.nodes[node].children.iter().copied().find(|&c| matches!(&self.nodes[c].kind, Kind::Literal(l) if l == text))
    }

    pub(crate) fn has_literals(&self, node: usize) -> bool {
        self.nodes[node].children.iter().any(|&c| matches!(self.nodes[c].kind, Kind::Literal(_)))
    }

    pub(crate) fn arguments(&self, node: usize) -> impl Iterator<Item = usize> + '_ {
        let children = &self.nodes[node].children;
        children.iter().enumerate().filter_map(move |(i, &c)| {
            let Kind::Argument { name, .. } = &self.nodes[c].kind else { return None };
            let first = !children[..i].iter().any(|&o| matches!(&self.nodes[o].kind, Kind::Argument { name: n, .. } if n == name));
            first.then_some(c)
        })
    }
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
            match ArgType::read(read_varint(input)?, input)? {
                Some(ty) => {
                    if flags & FLAG_CUSTOM_SUGGESTIONS != 0 {
                        String::read(input)?;
                    }
                    Kind::Argument { name, ty }
                }
                None => Kind::Root,
            }
        }
        _ => Kind::Root,
    };
    Ok(Node { kind, children, redirect, executable: flags & FLAG_EXECUTABLE != 0, restricted: flags & FLAG_RESTRICTED != 0 })
}

#[cfg(test)]
#[path = "command_tree_tests.rs"]
pub(crate) mod tests;
