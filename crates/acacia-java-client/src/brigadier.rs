//! Brigadier's `CommandDispatcher.parse` over the server's tree, and what vanilla reads from the result:
//! `SignableCommand.of` (through `ArgumentVisitor`, stopping at redirects back to the root, so `execute ... run`
//! signs nothing after `run`) and `ClientPacketListener.verifyCommand`.

use crate::arg_types::{ArgType, Source};
use crate::command_reader::{Fail, Reader};
use crate::command_tree::{CommandTree, Kind};

/// The client's permissions as the parse sees them.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Permissions {
    pub source: Source,
    /// `RESTRICTED_COMMAND`: granted to typed commands, not to `sendUnattendedCommand`'s check.
    pub restricted: bool,
}

/// `CommandContextBuilder`, as far as the visitor and validity check read it.
#[derive(Debug, Clone)]
struct Context {
    root: usize,
    nodes: Vec<usize>,
    /// `withArgument` puts by name; a later argument of the same name replaces the range.
    arguments: Vec<(String, (usize, usize))>,
    command: bool,
    child: Option<Box<Context>>,
}

impl Context {
    fn new(root: usize) -> Self {
        Self { root, nodes: Vec::new(), arguments: Vec::new(), command: false, child: None }
    }

    fn put(&mut self, name: &str, range: (usize, usize)) {
        match self.arguments.iter_mut().find(|(n, _)| n == name) {
            Some(entry) => entry.1 = range,
            None => self.arguments.push((name.to_owned(), range)),
        }
    }
}

/// `ParseResults`.
#[derive(Debug, Clone)]
struct Parse {
    context: Context,
    cursor: usize,
    errors: bool,
}

/// `ClientPacketListener.CommandCheckResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandCheck {
    NoIssues,
    ParseErrors,
    SignatureRequired,
    PermissionsRequired,
}

impl CommandTree {
    fn parse(&self, units: &[u16], permissions: Permissions) -> Parse {
        let reader = Reader::new(units);
        self.parse_nodes(self.root, &reader, &Context::new(self.root), permissions)
    }

    /// `CommandDispatcher.parseNodes`.
    fn parse_nodes(&self, node: usize, original: &Reader, so_far: &Context, permissions: Permissions) -> Parse {
        let mut errors = false;
        let mut potentials: Vec<Parse> = Vec::new();
        for child in self.relevant(node, original) {
            let n = self.node(child);
            if n.restricted && !permissions.restricted {
                continue;
            }
            let (mut context, mut reader) = (so_far.clone(), original.clone());
            if self.parse_child(child, &mut reader, &mut context, permissions.source).is_err() || (reader.can_read() && !reader.at(' ')) {
                errors = true;
                continue;
            }
            context.command = n.executable;
            if !reader.can_read_n(if n.redirect.is_none() { 2 } else { 1 }) {
                potentials.push(Parse { context, cursor: reader.cursor, errors: false });
                continue;
            }
            reader.skip();
            if let Some(target) = n.redirect {
                let parse = self.parse_nodes(target, &reader, &Context::new(target), permissions);
                context.child = Some(Box::new(parse.context));
                return Parse { context, cursor: parse.cursor, errors: parse.errors };
            }
            potentials.push(self.parse_nodes(child, &reader, &context, permissions));
        }
        // A stable sort: fully read first, then error-free.
        potentials.sort_by_key(|p| (p.cursor < original.len(), p.errors));
        potentials.into_iter().next().unwrap_or_else(|| Parse { context: so_far.clone(), cursor: original.cursor, errors })
    }

    /// `CommandNode.getRelevantNodes`: the literal matching the next word alone, else every argument.
    fn relevant(&self, node: usize, r: &Reader) -> Vec<usize> {
        if !self.has_literals(node) {
            return self.arguments(node).collect();
        }
        let mut word = r.clone();
        let text = word.read_word();
        match self.literal(node, &text) {
            Some(literal) => vec![literal],
            None => self.arguments(node).collect(),
        }
    }

    fn parse_child(&self, child: usize, r: &mut Reader, context: &mut Context, source: Source) -> Result<(), Fail> {
        let start = r.cursor;
        match &self.node(child).kind {
            Kind::Literal(literal) => {
                let units: Vec<u16> = literal.encode_utf16().collect();
                if !r.can_read_n(units.len()) || r.text(start, start + units.len()) != *literal {
                    return Err(Fail);
                }
                r.cursor += units.len();
                if r.can_read() && !r.at(' ') {
                    r.cursor = start;
                    return Err(Fail);
                }
            }
            Kind::Argument { name, ty } => {
                ty.parse(r, source)?;
                context.put(name, (start, r.cursor));
            }
            Kind::Root => return Err(Fail),
        }
        context.nodes.push(child);
        Ok(())
    }

    /// `SignableCommand.of(parse(command))`: each parsed message argument's name and text, in order.
    pub(crate) fn signable(&self, command: &str, permissions: Permissions) -> Vec<(String, String)> {
        let units: Vec<u16> = command.encode_utf16().collect();
        let parse = self.parse(&units, permissions);
        let mut found = Vec::new();
        let mut context = Some(&parse.context);
        while let Some(c) = context {
            for &node in &c.nodes {
                let Kind::Argument { name, ty: ArgType::Message } = &self.node(node).kind else { continue };
                if let Some((_, (start, end))) = c.arguments.iter().find(|(n, _)| n == name) {
                    found.push((name.clone(), String::from_utf16_lossy(&units[*start..*end])));
                }
            }
            context = c.child.as_deref().filter(|child| child.root != self.root);
        }
        found
    }

    /// `verifyCommand`, as `sendUnattendedCommand` runs it for dialogs and clickable text.
    pub(crate) fn verify(&self, command: &str, permissions: Permissions) -> CommandCheck {
        let units: Vec<u16> = command.encode_utf16().collect();
        let valid = |parse: &Parse| {
            let mut last = &parse.context;
            while let Some(child) = &last.child {
                last = child;
            }
            parse.cursor == units.len() && !parse.errors && last.command
        };
        if !valid(&self.parse(&units, Permissions { restricted: true, ..permissions })) {
            CommandCheck::ParseErrors
        } else if !self.signable(command, Permissions { restricted: true, ..permissions }).is_empty() {
            CommandCheck::SignatureRequired
        } else if !valid(&self.parse(&units, Permissions { source: Source::default(), restricted: false })) {
            CommandCheck::PermissionsRequired
        } else {
            CommandCheck::NoIssues
        }
    }
}

#[cfg(test)]
#[path = "brigadier_tests.rs"]
mod tests;
