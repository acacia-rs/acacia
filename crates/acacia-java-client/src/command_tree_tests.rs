use acacia_java_proto::write_varint;

use super::*;

const EXECUTABLE: u8 = 4;

/// A node as `ClientboundCommandsPacket.Entry.write` puts it.
fn node(out: &mut Vec<u8>, flags: u8, children: &[i32], redirect: Option<i32>, stub: impl FnOnce(&mut Vec<u8>)) {
    out.push(flags | if redirect.is_some() { FLAG_REDIRECT } else { 0 });
    write_varint(children.len() as i32, out);
    children.iter().for_each(|&c| write_varint(c, out));
    redirect.into_iter().for_each(|r| write_varint(r, out));
    stub(out);
}

fn literal(name: &str) -> impl FnOnce(&mut Vec<u8>) + '_ {
    move |out| name.to_owned().write(out)
}

fn argument<'a>(name: &'a str, parser: i32, properties: &[u8]) -> impl FnOnce(&mut Vec<u8>) + 'a {
    let properties = properties.to_vec();
    move |out| {
        name.to_owned().write(out);
        write_varint(parser, out);
        out.extend(properties);
    }
}

/// `msg <targets> <message>`, `tell` → `msg`, `tp <location>`, `say <message>`, `give <count>`, `execute run` →
/// root, `kick <reason: greedy string>`.
fn tree() -> CommandTree {
    let mut out = Vec::new();
    write_varint(13, &mut out);
    node(&mut out, 0, &[1, 4, 5, 7, 9, 11], None, |_| {});
    node(&mut out, TYPE_LITERAL, &[2], None, literal("msg"));
    node(&mut out, TYPE_ARGUMENT, &[3], None, argument("targets", parser::ENTITY, &[0]));
    node(&mut out, TYPE_ARGUMENT | EXECUTABLE, &[], None, argument("message", parser::MESSAGE, &[]));
    node(&mut out, TYPE_LITERAL, &[], Some(1), literal("tell"));
    node(&mut out, TYPE_LITERAL, &[6], None, literal("tp"));
    node(&mut out, TYPE_ARGUMENT | EXECUTABLE | FLAG_CUSTOM_SUGGESTIONS, &[], None, |out| {
        argument("location", parser::VEC3, &[])(out);
        "minecraft:ask_server".to_owned().write(out);
    });
    node(&mut out, TYPE_LITERAL, &[8], None, literal("say"));
    node(&mut out, TYPE_ARGUMENT | EXECUTABLE, &[], None, argument("message", parser::MESSAGE, &[]));
    node(&mut out, TYPE_LITERAL, &[10], None, literal("give"));
    // Integer with a minimum of 1: flags 1, then the int.
    node(&mut out, TYPE_ARGUMENT | EXECUTABLE, &[], None, argument("count", parser::INTEGER, &[1, 0, 0, 0, 1]));
    node(&mut out, TYPE_LITERAL, &[12], None, literal("execute"));
    node(&mut out, TYPE_LITERAL, &[], Some(0), literal("run"));
    write_varint(0, &mut out);
    let mut input = &out[..];
    let tree = CommandTree::read(&mut input).unwrap();
    assert!(input.is_empty());
    tree
}

fn signed(name: &str, value: &str) -> Vec<(String, String)> {
    vec![(name.to_owned(), value.to_owned())]
}

#[test]
fn message_arguments_are_signable_through_selectors_and_redirects() {
    let tree = tree();
    assert_eq!(tree.signable("msg @a[name=Bob, tag=x] hello there"), signed("message", "hello there"));
    assert_eq!(tree.signable("tell Bob hi"), signed("message", "hi"));
    assert_eq!(tree.signable("say  two  spaces "), signed("message", " two  spaces "));
    assert_eq!(tree.signable("execute run say hi"), signed("message", "hi"));
}

#[test]
fn commands_without_message_arguments_have_none() {
    let tree = tree();
    assert!(tree.signable("tp ~ ~1 ~").is_empty());
    assert!(tree.signable("give 5").is_empty());
    assert!(tree.signable("msg").is_empty());
    assert!(tree.signable("unknown thing").is_empty());
}

#[test]
fn tokens_respect_brackets_and_quotes() {
    assert_eq!(token("@e[type=cow, limit=1] rest"), Some(21));
    assert_eq!(token("\"a b\\\" c\" rest"), Some(9));
    assert_eq!(token(" leading"), None);
    assert_eq!(extent(parser::VEC3, false, "1 2 3 4"), Some(5));
    assert_eq!(extent(parser::VEC3, false, "1 2"), None);
}

#[test]
fn unknown_argument_types_fail_the_tree() {
    let mut out = Vec::new();
    write_varint(1, &mut out);
    node(&mut out, TYPE_ARGUMENT, &[], None, argument("x", parser::LAST + 1, &[]));
    assert!(CommandTree::read(&mut &out[..]).is_err());
}
