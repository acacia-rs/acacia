use acacia_bot::proto::packets::{AvailableCommandsCommandDataItem, AvailableCommandsCommandDataItemOverloadsItem, AvailableCommandsDynamicEnumsItem, AvailableCommandsEnumsItem};
use acacia_bot::proto::types::CommandFlags;

use super::*;

fn param(name: &str, value_type: u16, enum_type: EnumType, optional: bool) -> Param {
    Param { parameter_name: name.to_owned(), value_type, enum_type, optional, options: CommandFlags::default() }
}

fn packet() -> AvailableCommands {
    let command = |name: &str, alias: i32, overloads: Vec<Vec<Param>>| AvailableCommandsCommandDataItem {
        name: name.to_owned(),
        description: String::new(),
        flags: 0,
        permission_level: "any".to_owned(),
        alias,
        chained_subcommand_offsets: Vec::new(),
        overloads: overloads.into_iter().map(|parameters| AvailableCommandsCommandDataItemOverloadsItem { chaining: false, parameters }).collect(),
    };
    AvailableCommands {
        values_len: 6,
        enum_values: ["tell", "w", "msg", "query", "survival", "creative"].map(str::to_owned).to_vec(),
        chained_subcommand_values: Vec::new(),
        suffixes: Vec::new(),
        enums: vec![
            AvailableCommandsEnumsItem { name: "TellAliases".into(), values: vec![0, 1, 2] },
            AvailableCommandsEnumsItem { name: "Query".into(), values: vec![3] },
            AvailableCommandsEnumsItem { name: "GameMode".into(), values: vec![4, 5] },
        ],
        chained_subcommands: Vec::new(),
        command_data: vec![
            command("tell", 0, vec![vec![param("target", TARGET, EnumType::Valid, false), param("message", MESSAGE, EnumType::Valid, false)]]),
            command("time", -1, vec![vec![param("query", 1, EnumType::Enum, false), param("what", 0, EnumType::SoftEnum, true)]]),
            command("gamemode", -1, vec![vec![param("mode", 2, EnumType::Enum, false), param("player", TARGET, EnumType::Valid, true)]]),
            command("tp", -1, vec![vec![param("destination", POSITION, EnumType::Valid, false)]]),
            command("help", -1, vec![vec![param("page", INT, EnumType::Valid, true)]]),
        ],
        dynamic_enums: vec![AvailableCommandsDynamicEnumsItem { name: "Times".into(), values: vec!["day".into(), "night".into()] }],
        enum_constraints: Vec::new(),
    }
}

fn usage(tree: &Tree, root_child: &str) -> Vec<String> {
    let node = tree.node(0).children.iter().copied().find(|&c| tree.node(c).name() == root_child).expect("command");
    tree.smart_usage(node).into_iter().map(|(_, u)| u).collect()
}

#[test]
fn commands_aliases_and_parameters_become_nodes() {
    let tree = tree(&packet());
    let roots: Vec<&str> = tree.node(0).children.iter().map(|&c| tree.node(c).name()).collect();
    assert_eq!(roots, ["tell", "w", "msg", "time", "gamemode", "tp", "help"]);
    assert_eq!(usage(&tree, "w"), ["<target> <message>"]);
    // A one-value enum is a subcommand; the soft enum after it is optional.
    assert_eq!(usage(&tree, "time"), ["query [<what>]"]);
    assert_eq!(usage(&tree, "gamemode"), ["<mode> [<player>]"]);
    assert_eq!(usage(&tree, "tp"), ["<destination:x> <destination:y> <destination:z>"]);
    let help = tree.node(0).children[6];
    assert!(tree.node(help).executable);
}

#[test]
fn enum_values_and_selectors_complete() {
    let tree = tree(&packet());
    let input: Vec<u16> = "/gamemode c".encode_utf16().collect();
    let found = tree.completions(&tree.parse(&input, 1), input.len());
    assert_eq!(found.list.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(), ["creative"]);
    let input: Vec<u16> = "/tell @".encode_utf16().collect();
    let found = tree.completions(&tree.parse(&input, 1), input.len());
    assert_eq!(found.list.len(), SELECTORS.len());
}
