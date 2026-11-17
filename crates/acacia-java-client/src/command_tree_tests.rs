use acacia_java_proto::write_varint;

use super::*;

pub(crate) const ENTITY: i32 = 6;
pub(crate) const VEC3: i32 = 10;
pub(crate) const MESSAGE: i32 = 20;
pub(crate) const INTEGER: i32 = 3;

/// Builds a `ClientboundCommandsPacket` body node by node, as `Entry.write` puts them.
#[derive(Default)]
pub(crate) struct Packet(Vec<u8>, i32);

impl Packet {
    fn node(&mut self, flags: u8, children: &[i32], redirect: Option<i32>) -> &mut Vec<u8> {
        self.1 += 1;
        let out = &mut self.0;
        out.push(flags | if redirect.is_some() { FLAG_REDIRECT } else { 0 });
        write_varint(children.len() as i32, out);
        children.iter().for_each(|&c| write_varint(c, out));
        redirect.into_iter().for_each(|r| write_varint(r, out));
        out
    }

    pub(crate) fn root(&mut self, children: &[i32]) -> &mut Self {
        self.node(0, children, None);
        self
    }

    pub(crate) fn literal(&mut self, name: &str, flags: u8, children: &[i32], redirect: Option<i32>) -> &mut Self {
        name.to_owned().write(self.node(TYPE_LITERAL | flags, children, redirect));
        self
    }

    pub(crate) fn argument(&mut self, name: &str, flags: u8, children: &[i32], parser: i32, properties: &[u8]) -> &mut Self {
        self.argument_to(name, flags, children, None, parser, properties)
    }

    pub(crate) fn argument_to(&mut self, name: &str, flags: u8, children: &[i32], redirect: Option<i32>, parser: i32, properties: &[u8]) -> &mut Self {
        let out = self.node(TYPE_ARGUMENT | flags, children, redirect);
        name.to_owned().write(out);
        write_varint(parser, out);
        out.extend(properties);
        self
    }

    pub(crate) fn suggestions(&mut self, provider: &str) -> &mut Self {
        provider.to_owned().write(&mut self.0);
        self
    }

    pub(crate) fn tree(&mut self, root: i32) -> CommandTree {
        let mut out = Vec::new();
        write_varint(self.1, &mut out);
        out.extend(&self.0);
        write_varint(root, &mut out);
        let mut input = &out[..];
        let tree = CommandTree::read(&mut input).unwrap();
        assert!(input.is_empty());
        tree
    }
}

#[test]
fn nodes_keep_their_kind_flags_and_links() {
    let tree = Packet::default()
        .root(&[1, 3])
        .literal("tp", 0, &[2], None)
        .argument("location", FLAG_EXECUTABLE | FLAG_CUSTOM_SUGGESTIONS, &[], VEC3, &[])
        .suggestions("minecraft:ask_server")
        .literal("give", FLAG_RESTRICTED, &[4], Some(0))
        .argument("count", FLAG_EXECUTABLE, &[], INTEGER, &[1, 0, 0, 0, 1])
        .tree(0);
    assert_eq!(tree.node(2).kind, Kind::Argument { name: "location".into(), ty: ArgType::Position { int: false } });
    assert!(tree.node(2).executable && !tree.node(1).executable);
    assert_eq!(tree.node(4).kind, Kind::Argument { name: "count".into(), ty: ArgType::Integer(Some(1), None) });
    assert!(tree.node(3).restricted && tree.node(3).redirect == Some(0));
    assert_eq!(tree.literal(0, "give"), Some(3));
    assert!(tree.has_literals(0) && !tree.has_literals(1));
}

#[test]
fn an_unknown_argument_type_drops_only_its_node() {
    let tree = Packet::default()
        .root(&[1, 2])
        .argument("x", FLAG_EXECUTABLE, &[], 9999, &[])
        .literal("say", 0, &[3], None)
        .argument("message", FLAG_EXECUTABLE, &[], MESSAGE, &[])
        .tree(0);
    assert_eq!(tree.node(1).kind, Kind::Root);
    assert_eq!(tree.node(0).children, [2]);
    assert_eq!(tree.node(3).kind, Kind::Argument { name: "message".into(), ty: ArgType::Message });
}

#[test]
fn bad_links_fail_the_tree() {
    let mut out = Vec::new();
    write_varint(1, &mut out);
    out.push(0);
    write_varint(1, &mut out);
    write_varint(5, &mut out);
    write_varint(0, &mut out);
    assert!(CommandTree::read(&mut &out[..]).is_err());
}

#[test]
fn later_arguments_of_a_name_are_shadowed() {
    let tree = Packet::default()
        .root(&[1, 2, 3])
        .argument("a", 0, &[], INTEGER, &[0])
        .argument("a", 0, &[], ENTITY, &[0])
        .argument("b", 0, &[], ENTITY, &[0])
        .tree(0);
    assert_eq!(tree.arguments(0).collect::<Vec<_>>(), [1, 3]);
}
