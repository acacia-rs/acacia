use super::*;
use crate::command_tree::tests::{ENTITY, MESSAGE, Packet, VEC3};

const EXECUTABLE: u8 = 4;
const RESTRICTED: u8 = 32;

const OP: Permissions = Permissions { source: Source { selectors: true }, restricted: true };
const PLAYER: Permissions = Permissions { source: Source { selectors: false }, restricted: true };

/// `msg <targets> <message>`, `tell` → `msg`, `say <message>`, `execute run` → root, `execute as <targets>` →
/// `execute`, a restricted `stop`, `tp <location>`.
fn tree() -> CommandTree {
    Packet::default()
        .root(&[1, 4, 5, 7, 11, 12])
        .literal("msg", 0, &[2], None)
        .argument("targets", 0, &[3], ENTITY, &[0])
        .argument("message", EXECUTABLE, &[], MESSAGE, &[])
        .literal("tell", 0, &[], Some(1))
        .literal("say", 0, &[6], None)
        .argument("message", EXECUTABLE, &[], MESSAGE, &[])
        .literal("execute", 0, &[8, 9], None)
        .literal("run", 0, &[], Some(0))
        .literal("as", 0, &[10], None)
        .argument_to("targets", 0, &[], Some(7), ENTITY, &[0])
        .literal("stop", EXECUTABLE | RESTRICTED, &[], None)
        .literal("tp", 0, &[13], None)
        .argument("location", EXECUTABLE, &[], VEC3, &[])
        .tree(0)
}

fn signed(value: &str) -> Vec<(String, String)> {
    vec![("message".to_owned(), value.to_owned())]
}

#[test]
fn message_arguments_are_signed_through_selectors_and_redirects() {
    let tree = tree();
    assert_eq!(tree.signable("msg @a[name=Bob, tag=x] hello there", OP), signed("hello there"));
    assert_eq!(tree.signable("tell Bob hi", PLAYER), signed("hi"));
    assert_eq!(tree.signable("say  two  spaces ", PLAYER), signed(" two  spaces "));
}

#[test]
fn selectors_need_the_permission() {
    let tree = tree();
    assert!(tree.signable("msg @a hi", PLAYER).is_empty(), "the target fails to parse, so no message is reached");
    assert_eq!(tree.signable("msg Bob hi @a", PLAYER), signed("hi @a"));
}

#[test]
fn redirects_to_the_root_end_the_signed_part() {
    let tree = tree();
    assert!(tree.signable("execute run say hi", OP).is_empty());
    assert!(tree.signable("execute as @s run say hi", OP).is_empty());
}

#[test]
fn commands_without_message_arguments_have_none() {
    let tree = tree();
    assert!(tree.signable("tp ~ ~1 ~", PLAYER).is_empty());
    assert!(tree.signable("msg", PLAYER).is_empty());
    assert!(tree.signable("unknown thing", PLAYER).is_empty());
}

#[test]
fn unattended_commands_are_verified_as_vanilla_does() {
    let tree = tree();
    assert_eq!(tree.verify("say hi", PLAYER), CommandCheck::SignatureRequired);
    assert_eq!(tree.verify("tp ~ ~ ~", PLAYER), CommandCheck::NoIssues);
    assert_eq!(tree.verify("stop", PLAYER), CommandCheck::PermissionsRequired);
    assert_eq!(tree.verify("execute as @s run tp 1 2 3", OP), CommandCheck::PermissionsRequired, "selectors need op");
    assert_eq!(tree.verify("tp ~", PLAYER), CommandCheck::ParseErrors);
    assert_eq!(tree.verify("tp ~ ~ ~ ", PLAYER), CommandCheck::ParseErrors, "a trailing space is left unread");
    assert_eq!(tree.verify("nope", PLAYER), CommandCheck::ParseErrors);
}

/// A Paper 26.3 + Grim server's `ClientboundCommandsPacket` body for a player, and for the same player opped.
fn paper(op: bool) -> CommandTree {
    let body: &[u8] = if op { include_bytes!("../data/paper_commands_op.bin") } else { include_bytes!("../data/paper_commands.bin") };
    let mut input = body;
    let tree = CommandTree::read(&mut input).unwrap();
    assert!(input.is_empty());
    assert_eq!(tree.nodes.iter().filter(|n| n.kind == Kind::Root).count(), 1, "every argument type is known");
    tree
}

fn signed_as(name: &str, value: &str) -> Vec<(String, String)> {
    vec![(name.to_owned(), value.to_owned())]
}

#[test]
fn paper_player_tree_signs_messages_through_aliases() {
    let tree = paper(false);
    assert_eq!(tree.signable("msg AcaciaJava hello there", PLAYER), signed("hello there"));
    assert_eq!(tree.signable("w AcaciaJava hi", PLAYER), signed("hi"));
    assert_eq!(tree.signable("minecraft:tell AcaciaJava hi", PLAYER), signed("hi"));
    assert_eq!(tree.signable("me waves", PLAYER), signed_as("action", "waves"));
    assert_eq!(tree.signable("teammsg hi", PLAYER), signed("hi"));
    assert!(tree.signable("msg @a hi", PLAYER).is_empty(), "no selectors without op");
    assert_eq!(tree.verify("msg AcaciaJava hi", PLAYER), CommandCheck::SignatureRequired);
    assert_eq!(tree.verify("say hi", PLAYER), CommandCheck::ParseErrors);
}

#[test]
fn paper_op_tree_parses_vanilla_commands() {
    let tree = paper(true);
    assert_eq!(tree.signable("msg @a[distance=..5,limit=1] hi", OP), signed("hi"));
    assert_eq!(tree.signable("say hello @a", OP), signed("hello @a"));
    assert!(tree.signable("execute as @a at @s run say hi", OP).is_empty());
    assert!(tree.signable("execute if entity @s[type=minecraft:cow] run me x", OP).is_empty());
    assert!(tree.signable(r#"tellraw @a {text:"x"}"#, OP).is_empty());
    assert_eq!(tree.verify("say hi", OP), CommandCheck::SignatureRequired);
    assert_eq!(tree.verify("give @s minecraft:diamond_sword[damage=5] 1", OP), CommandCheck::PermissionsRequired);
    assert_eq!(tree.verify("tp @s ~ ~1 ~", OP), CommandCheck::PermissionsRequired);
    assert_eq!(tree.verify("setblock 1 2 3 minecraft:oak_stairs[facing=east]", OP), CommandCheck::PermissionsRequired);
    assert_eq!(tree.verify("me", OP), CommandCheck::ParseErrors);
    assert_eq!(tree.verify("give @s", OP), CommandCheck::ParseErrors);
}
