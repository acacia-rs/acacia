//! The command tree: Brigadier's `RootCommandNode`, `LiteralCommandNode` and
//! `ArgumentCommandNode` without commands (only whether a node executes), requirements (all
//! usable) or redirects.

use serde_json::Value;

use super::error::{Error, Kind};
use super::reader::Reader;

#[derive(Clone, Debug, PartialEq)]
pub enum ArgType {
    /// `IntegerArgumentType.integer(min, max)`.
    Integer { min: i32, max: i32 },
    /// `FloatArgumentType.floatArg(min, max)`.
    Float { min: f32, max: f32 },
    /// `BoolArgumentType.bool()`.
    Bool,
    /// `StringArgumentType.word()`.
    Word,
    /// `StringArgumentType.string()`.
    Quotable,
    /// `StringArgumentType.greedyString()`.
    Greedy,
    /// Everything up to the next space: acacia's type for Bedrock parameters Brigadier has no type
    /// for (selectors, coordinates, ids).
    Token,
}

impl ArgType {
    /// `ArgumentType.parse`.
    pub fn parse(&self, r: &mut Reader) -> Result<(), Error> {
        let start = r.cursor;
        match *self {
            ArgType::Integer { min, max } => {
                let v = r.read_int()?;
                if v < min || v > max {
                    r.cursor = start;
                    return Err(r.error(if v < min { Kind::IntegerTooLow { found: v, min } } else { Kind::IntegerTooHigh { found: v, max } }));
                }
            }
            ArgType::Float { min, max } => {
                let v = r.read_float()?;
                if v < min || v > max {
                    r.cursor = start;
                    return Err(r.error(if v < min { Kind::FloatTooLow { found: v, min } } else { Kind::FloatTooHigh { found: v, max } }));
                }
            }
            ArgType::Bool => {
                r.read_bool()?;
            }
            ArgType::Word => {
                r.read_unquoted();
            }
            ArgType::Quotable => {
                r.read_string()?;
            }
            ArgType::Greedy => r.cursor = r.string.len(),
            ArgType::Token => {
                r.read_token();
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    Root,
    Literal(String),
    /// An argument; `suggestions` are its custom ones (`SharedSuggestionProvider.suggest` over
    /// the list), else the type's own.
    Argument { name: String, ty: ArgType, suggestions: Option<Vec<String>> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub kind: NodeKind,
    /// Has a command: input ending here runs.
    pub executable: bool,
    /// In insertion order (`CommandNode.children`, a `LinkedHashMap`).
    pub children: Vec<usize>,
}

impl Node {
    /// `getName`.
    pub fn name(&self) -> &str {
        match &self.kind {
            NodeKind::Root => "",
            NodeKind::Literal(l) => l,
            NodeKind::Argument { name, .. } => name,
        }
    }

    pub fn is_literal(&self) -> bool {
        matches!(self.kind, NodeKind::Literal(_))
    }

    /// `getUsageText`.
    pub fn usage(&self) -> String {
        match &self.kind {
            NodeKind::Root => String::new(),
            NodeKind::Literal(l) => l.clone(),
            NodeKind::Argument { name, .. } => format!("<{name}>"),
        }
    }
}

/// A `CommandDispatcher`'s tree; node 0 is the root.
#[derive(Clone, Debug, PartialEq)]
pub struct Tree {
    pub nodes: Vec<Node>,
}

impl Default for Tree {
    fn default() -> Tree {
        Tree { nodes: vec![Node { kind: NodeKind::Root, executable: false, children: Vec::new() }] }
    }
}

impl Tree {
    pub fn node(&self, i: usize) -> &Node {
        &self.nodes[i]
    }

    /// `parent.addChild`: a child of the same name is reused (and made executable if this one is).
    pub fn add(&mut self, parent: usize, kind: NodeKind, executable: bool) -> usize {
        let node = Node { kind, executable, children: Vec::new() };
        if let Some(&existing) = self.nodes[parent].children.iter().find(|&&c| self.nodes[c].name() == node.name()) {
            self.nodes[existing].executable |= executable;
            return existing;
        }
        self.nodes.push(node);
        let i = self.nodes.len() - 1;
        self.nodes[parent].children.push(i);
        i
    }

    /// The root's children from JSON: `{"literal": name}` or `{"argument": name, "type": ..., "min",
    /// "max", "suggest": [...]}`, each with `"exec"` and `"children"`.
    pub fn from_json(children: &Value) -> Tree {
        let mut tree = Tree::default();
        tree.add_json(0, children);
        tree
    }

    fn add_json(&mut self, parent: usize, children: &Value) {
        for c in children.as_array().into_iter().flatten() {
            let kind = match (c["literal"].as_str(), c["argument"].as_str()) {
                (Some(l), _) => NodeKind::Literal(l.to_owned()),
                (_, Some(name)) => {
                    let suggestions = c["suggest"].as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect());
                    NodeKind::Argument { name: name.to_owned(), ty: arg_type(c), suggestions }
                }
                _ => continue,
            };
            let i = self.add(parent, kind, c["exec"].as_bool().unwrap_or(false));
            self.add_json(i, &c["children"]);
        }
    }
}

fn arg_type(c: &Value) -> ArgType {
    let int = |k: &str, d: i32| c[k].as_i64().map_or(d, |v| v as i32);
    let float = |k: &str, d: f32| c[k].as_f64().map_or(d, |v| v as f32);
    match c["type"].as_str().unwrap_or("token") {
        "integer" => ArgType::Integer { min: int("min", i32::MIN), max: int("max", i32::MAX) },
        "float" => ArgType::Float { min: float("min", -f32::MAX), max: float("max", f32::MAX) },
        "bool" => ArgType::Bool,
        "word" => ArgType::Word,
        "string" => ArgType::Quotable,
        "greedy" => ArgType::Greedy,
        _ => ArgType::Token,
    }
}
