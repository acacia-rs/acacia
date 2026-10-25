//! Bedrock's `AvailableCommands` as the Brigadier-like tree the Java chat screen completes from
//! (`acacia_ui::java::command::Tree`): each command and alias a root literal, each overload a chain
//! of its parameters, executable where the rest of the overload is optional.
//!
//! Parameters: a one-value enum is a literal (a subcommand), a longer enum or a soft enum a word
//! suggesting its values; int and float are Brigadier's numbers; message, raw text, JSON and
//! command take the rest of the line; string is a quotable string; positions are three words;
//! targets suggest the selectors; everything else (suffixed values, ids, ranges) is one word.

use acacia_bot::proto::packets::{
    AvailableCommands, AvailableCommandsCommandDataItemOverloadsItemParametersItem as Param,
    AvailableCommandsCommandDataItemOverloadsItemParametersItemEnumType as EnumType,
};
use acacia_ui::java::command::{ArgType, NodeKind, Tree};

const SELECTORS: [&str; 6] = ["@a", "@e", "@p", "@r", "@s", "@initiator"];

/// `value_type`s (tools/codegen/data/protocol.json's command arg types).
const INT: u16 = 1;
const FLOAT: u16 = 3;
const TARGET: u16 = 8;
const WILDCARD_TARGET: u16 = 10;
const STRING: u16 = 44;
const BLOCK_POSITION: u16 = 52;
const POSITION: u16 = 53;
const MESSAGE: u16 = 55;
const RAW_TEXT: u16 = 58;
const JSON: u16 = 62;
const COMMAND: u16 = 75;

fn argument(name: &str, ty: ArgType, suggestions: Option<Vec<String>>) -> NodeKind {
    NodeKind::Argument { name: name.to_owned(), ty, suggestions }
}

/// The nodes one parameter becomes, in order.
fn nodes(p: &Param, packet: &AvailableCommands) -> Vec<NodeKind> {
    let index = usize::from(p.value_type);
    let name = p.parameter_name.as_str();
    match p.enum_type {
        EnumType::Enum => {
            let values: Vec<String> = packet.enums.get(index).into_iter().flat_map(|e| &e.values).filter_map(|&v| packet.enum_values.get(v as usize).cloned()).collect();
            match values.as_slice() {
                [only] => vec![NodeKind::Literal(only.clone())],
                _ => vec![argument(name, ArgType::Token, Some(values))],
            }
        }
        EnumType::SoftEnum => vec![argument(name, ArgType::Token, packet.dynamic_enums.get(index).map(|e| e.values.clone()))],
        EnumType::Valid => match p.value_type {
            INT => vec![argument(name, ArgType::Integer { min: i32::MIN, max: i32::MAX }, None)],
            FLOAT => vec![argument(name, ArgType::Float { min: -f32::MAX, max: f32::MAX }, None)],
            STRING => vec![argument(name, ArgType::Quotable, None)],
            MESSAGE | RAW_TEXT | JSON | COMMAND => vec![argument(name, ArgType::Greedy, None)],
            TARGET | WILDCARD_TARGET => vec![argument(name, ArgType::Token, Some(SELECTORS.map(str::to_owned).to_vec()))],
            POSITION | BLOCK_POSITION => ["x", "y", "z"].map(|c| argument(&format!("{name}:{c}"), ArgType::Token, None)).to_vec(),
            _ => vec![argument(name, ArgType::Token, None)],
        },
        _ => vec![argument(name, ArgType::Token, None)],
    }
}

/// The tree of every command the server lets the player run.
pub fn tree(packet: &AvailableCommands) -> Tree {
    let mut tree = Tree::default();
    for command in &packet.command_data {
        let mut names = vec![command.name.clone()];
        let aliases = usize::try_from(command.alias).ok().and_then(|a| packet.enums.get(a));
        for &v in aliases.into_iter().flat_map(|e| &e.values) {
            if let Some(alias) = packet.enum_values.get(v as usize).filter(|a| !names.contains(a)) {
                names.push(alias.clone());
            }
        }
        for name in &names {
            let root = tree.add(0, NodeKind::Literal(name.clone()), command.overloads.is_empty());
            for overload in &command.overloads {
                let params = &overload.parameters;
                let rest_optional = |i: usize| params[i..].iter().all(|p| p.optional);
                if rest_optional(0) {
                    tree.add(0, NodeKind::Literal(name.clone()), true);
                }
                let mut at = root;
                for (i, p) in params.iter().enumerate() {
                    let kinds = nodes(p, packet);
                    let last = kinds.len() - 1;
                    for (k, kind) in kinds.into_iter().enumerate() {
                        at = tree.add(at, kind, k == last && rest_optional(i + 1));
                    }
                }
            }
        }
    }
    tree
}

#[cfg(test)]
mod tests;
