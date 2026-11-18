//! `ClientboundCommandsPacket`: a Brigadier tree as a flat node list. Each entry is a flags byte (type in bits 0-1:
//! root 0, literal 1, argument 2; 4 executable, 8 redirect, 16 custom suggestions, 32 restricted), the child
//! indices as a VarInt array, the redirect index when flagged, then the literal's text or the argument's name,
//! parser and options. After the list comes the root's index.

use crate::registry::COMMAND_ARGUMENT_TYPE;
use crate::types::var_int_enum;
use crate::wire::read_len;
use crate::{Error, VarInt, Wire, read_varint, write_varint};

const TYPE_MASK: u8 = 3;
const EXECUTABLE: u8 = 4;
const REDIRECT: u8 = 8;
const SUGGESTIONS: u8 = 16;
const RESTRICTED: u8 = 32;

var_int_enum!(
    /// `StringArgumentType.StringType`.
    StringKind, "string kind",
    [SingleWord, Quotable, Greedy]
);

/// The argument types with a known option layout (`ArgumentTypeInfo.serializeToNetwork`, from javap). Numeric
/// bounds are a flags byte (1 min, 2 max) and the present bounds; entity is a flags byte (1 single, 2 players
/// only); the rest take no options.
#[derive(Debug, Clone, PartialEq)]
pub enum Parser {
    Bool,
    Integer { min: Option<i32>, max: Option<i32> },
    Long { min: Option<i64>, max: Option<i64> },
    Float { min: Option<f32>, max: Option<f32> },
    Double { min: Option<f64>, max: Option<f64> },
    String(StringKind),
    Entity { single: bool, players_only: bool },
    GameProfile,
    BlockPos,
    ColumnPos,
    Vec3,
    Vec2,
    Component,
    Message,
}

impl Parser {
    fn name(&self) -> &'static str {
        match self {
            Self::Bool => "brigadier:bool",
            Self::Integer { .. } => "brigadier:integer",
            Self::Long { .. } => "brigadier:long",
            Self::Float { .. } => "brigadier:float",
            Self::Double { .. } => "brigadier:double",
            Self::String(_) => "brigadier:string",
            Self::Entity { .. } => "minecraft:entity",
            Self::GameProfile => "minecraft:game_profile",
            Self::BlockPos => "minecraft:block_pos",
            Self::ColumnPos => "minecraft:column_pos",
            Self::Vec3 => "minecraft:vec3",
            Self::Vec2 => "minecraft:vec2",
            Self::Component => "minecraft:component",
            Self::Message => "minecraft:message",
        }
    }
}

fn read_bounds<T: Wire>(input: &mut &[u8]) -> Result<(Option<T>, Option<T>), Error> {
    let flags = u8::read(input)?;
    let min = if flags & 1 != 0 { Some(T::read(input)?) } else { None };
    let max = if flags & 2 != 0 { Some(T::read(input)?) } else { None };
    Ok((min, max))
}

fn write_bounds<T: Wire>(min: &Option<T>, max: &Option<T>, out: &mut Vec<u8>) {
    (u8::from(min.is_some()) | u8::from(max.is_some()) << 1).write(out);
    min.iter().chain(max).for_each(|v| v.write(out));
}

impl Wire for Parser {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let id = read_varint(input)?;
        Ok(match COMMAND_ARGUMENT_TYPE.name(id).ok_or(Error::Invalid("argument type"))? {
            "brigadier:bool" => Self::Bool,
            "brigadier:integer" => read_bounds(input).map(|(min, max)| Self::Integer { min, max })?,
            "brigadier:long" => read_bounds(input).map(|(min, max)| Self::Long { min, max })?,
            "brigadier:float" => read_bounds(input).map(|(min, max)| Self::Float { min, max })?,
            "brigadier:double" => read_bounds(input).map(|(min, max)| Self::Double { min, max })?,
            "brigadier:string" => Self::String(StringKind::read(input)?),
            "minecraft:entity" => {
                let flags = u8::read(input)?;
                Self::Entity { single: flags & 1 != 0, players_only: flags & 2 != 0 }
            }
            "minecraft:game_profile" => Self::GameProfile,
            "minecraft:block_pos" => Self::BlockPos,
            "minecraft:column_pos" => Self::ColumnPos,
            "minecraft:vec3" => Self::Vec3,
            "minecraft:vec2" => Self::Vec2,
            "minecraft:component" => Self::Component,
            "minecraft:message" => Self::Message,
            _ => return Err(Error::Invalid("argument type without a known layout")),
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        write_varint(COMMAND_ARGUMENT_TYPE.id(self.name()).expect("generated registry has the argument type"), out);
        match self {
            Self::Integer { min, max } => write_bounds(min, max, out),
            Self::Long { min, max } => write_bounds(min, max, out),
            Self::Float { min, max } => write_bounds(min, max, out),
            Self::Double { min, max } => write_bounds(min, max, out),
            Self::String(kind) => kind.write(out),
            Self::Entity { single, players_only } => (u8::from(*single) | u8::from(*players_only) << 1).write(out),
            _ => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Root,
    Literal(String),
    /// `suggestions` names a custom suggestion provider (`minecraft:ask_server`, `minecraft:summonable_entities`...).
    Argument { name: String, parser: Parser, suggestions: Option<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandNode {
    pub kind: NodeKind,
    pub executable: bool,
    /// The client hides restricted nodes from players without permission.
    pub restricted: bool,
    pub redirect: Option<i32>,
    pub children: Vec<i32>,
}

impl CommandNode {
    pub fn new(kind: NodeKind) -> Self {
        Self { kind, executable: false, restricted: false, redirect: None, children: Vec::new() }
    }
}

impl Wire for CommandNode {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let flags = u8::read(input)?;
        let children = (0..read_len(input)?).map(|_| read_varint(input)).collect::<Result<_, _>>()?;
        let redirect = if flags & REDIRECT != 0 { Some(read_varint(input)?) } else { None };
        let kind = match flags & TYPE_MASK {
            0 => NodeKind::Root,
            1 => NodeKind::Literal(String::read(input)?),
            2 => NodeKind::Argument {
                name: String::read(input)?,
                parser: Parser::read(input)?,
                suggestions: if flags & SUGGESTIONS != 0 { Some(String::read(input)?) } else { None },
            },
            _ => return Err(Error::Invalid("command node type")),
        };
        Ok(Self { kind, executable: flags & EXECUTABLE != 0, restricted: flags & RESTRICTED != 0, redirect, children })
    }

    fn write(&self, out: &mut Vec<u8>) {
        let (kind, suggestions) = match &self.kind {
            NodeKind::Root => (0, false),
            NodeKind::Literal(_) => (1, false),
            NodeKind::Argument { suggestions, .. } => (2, suggestions.is_some()),
        };
        let flags = kind
            | if self.executable { EXECUTABLE } else { 0 }
            | if self.redirect.is_some() { REDIRECT } else { 0 }
            | if suggestions { SUGGESTIONS } else { 0 }
            | if self.restricted { RESTRICTED } else { 0 };
        flags.write(out);
        write_varint(self.children.len() as i32, out);
        self.children.iter().for_each(|&child| write_varint(child, out));
        if let Some(target) = self.redirect {
            write_varint(target, out);
        }
        match &self.kind {
            NodeKind::Root => {}
            NodeKind::Literal(text) => text.write(out),
            NodeKind::Argument { name, parser, suggestions } => {
                name.write(out);
                parser.write(out);
                if let Some(provider) = suggestions {
                    provider.write(out);
                }
            }
        }
    }
}

/// The whole tree; `root` indexes `nodes`.
#[derive(Debug, Clone, PartialEq)]
pub struct Commands {
    pub nodes: Vec<CommandNode>,
    pub root: i32,
}

impl Wire for Commands {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let nodes: Vec<CommandNode> = Vec::read(input)?;
        Ok(Self { nodes, root: VarInt::read(input)?.0 })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.nodes.write(out);
        write_varint(self.root, out);
    }
}
