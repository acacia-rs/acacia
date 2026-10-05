use std::collections::HashMap;

use crate::error::Error;
use crate::names::Names;
use crate::parse::Parser;
use crate::program::{Node, Program};
use crate::value::{Symbol, Value};

/// An entity variable, by the name after `variable.` up to the first member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Variable(pub(crate) u32);

/// A query, by the name after `query.`. Numbered from 0 in the order sources first used them, so a
/// host can keep a table indexed by `.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Query(pub u32);

/// A context variable, by the name after `context.`; numbered like [`Query`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Context(pub u32);

/// Array definitions for [`Compiler::compile_with`]: the name after `array.`, lowercase, to its
/// element expressions.
pub type Arrays = HashMap<String, Vec<String>>;

/// A pack's `min_engine_version`. Molang's rules changed over time, and every expression follows
/// the rules of the version its own pack declares; see the README, "Older packs".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Engine(pub u16, pub u16, pub u16);

impl Engine {
    pub const LATEST: Engine = Engine(u16::MAX, 0, 0);
}

/// Compiles sources and owns the names they share: strings, variables, queries and context
/// variables. Programs evaluate against any [`crate::Variables`] of the same compiler.
#[derive(Debug)]
pub struct Compiler {
    /// The rules for the sources compiled from now on: set it to the version of the pack they come from.
    pub engine: Engine,
    /// Reject `query.` names outside [`crate::QUERY_NAMES`], as BDS rejects names it cannot resolve.
    /// On by default; turn it off for a game newer than the list, or for queries of the host's own.
    pub documented_queries_only: bool,
    pub(crate) strings: Names,
    pub(crate) variables: Names,
    pub(crate) queries: Names,
    pub(crate) contexts: Names,
}

impl Default for Compiler {
    fn default() -> Compiler {
        let mut strings = Names::default();
        strings.intern("");
        Compiler { engine: Engine::LATEST, documented_queries_only: true, strings, variables: Names::default(), queries: Names::default(), contexts: Names::default() }
    }
}

impl Compiler {
    pub fn new() -> Compiler {
        Compiler::default()
    }

    pub fn compile(&mut self, source: &str) -> Result<Program, Error> {
        Parser::new(self, None).program(source)
    }

    /// Like [`Compiler::compile`], for a source that may index `arrays`.
    pub fn compile_with(&mut self, source: &str, arrays: &Arrays) -> Result<Program, Error> {
        Parser::new(self, Some(arrays)).program(source)
    }

    pub fn constant(value: f32) -> Program {
        Program { nodes: Box::new([Node::Const(Value::Num(value))]), lists: Box::new([]), root: 0, complex: false, temps: 0 }
    }

    /// Strings keep their case; only names are case-insensitive.
    pub fn symbol(&mut self, text: &str) -> Symbol {
        Symbol(self.strings.intern(text))
    }

    /// The symbol of a string some compiled source spells. A host holding a string that none does
    /// answers with [`Symbol::OTHER`]: no literal can equal it.
    pub fn find(&self, text: &str) -> Option<Symbol> {
        self.strings.find(text).map(Symbol)
    }

    /// Empty for [`Symbol::OTHER`].
    pub fn text(&self, symbol: Symbol) -> &str {
        if symbol == Symbol::OTHER { "" } else { self.strings.name(symbol.0) }
    }

    pub fn variable(&mut self, name: &str) -> Variable {
        Variable(self.variables.intern(&name.to_ascii_lowercase()))
    }

    /// The number of `query.<name>`, whether or not a source has used it yet.
    pub fn query(&mut self, name: &str) -> Query {
        Query(self.queries.intern(&name.to_ascii_lowercase()))
    }

    pub fn context(&mut self, name: &str) -> Context {
        Context(self.contexts.intern(&name.to_ascii_lowercase()))
    }

    pub fn query_name(&self, query: Query) -> &str {
        self.queries.name(query.0)
    }

    /// How many queries have been numbered so far.
    pub fn query_count(&self) -> usize {
        self.queries.len()
    }

    pub fn context_name(&self, context: Context) -> &str {
        self.contexts.name(context.0)
    }

    pub fn context_count(&self) -> usize {
        self.contexts.len()
    }
}
