//! Statements and the parser's plumbing; operators and names are in `parse_expr.rs`. Nodes are
//! emitted as they are parsed, with every name already resolved. The rules follow what BDS accepts
//! (`tests/oracle/*.bds`).

use crate::compiler::{Arrays, Compiler, Engine};
use crate::error::{Error, ErrorKind};
use crate::fold::{Folded, fold};
use crate::lex::{Token, tokens};
use crate::parse_expr::{LEVELS, Levels, OLD_LEVELS};
use crate::program::{List, Node, Op, Program};

/// The tallest tree a program may have; a long `a + b + c + ...` chain is as tall as it is long.
const MAX_HEIGHT: u16 = 512;

/// What a pack's engine version changes; each switch is where BDS's answers change
/// (`tests/oracle/versions@*.bds`).
pub(crate) struct Rules {
    pub(crate) levels: Levels,
    pub(crate) conditionals_chain_left: bool,
    /// A computed divisor loses its sign: `6 / v.d` is 3 for a `v.d` of -2.
    pub(crate) divides_by_magnitude: bool,
    /// The same, in a complex expression only; BDS fixed those later than simple ones.
    pub(crate) statements_divide_by_magnitude: bool,
    /// `(v.x = 3) + 1` compiles; later, only `==`, `!=`, `??` and the conditional take an assignment.
    pub(crate) assignments_are_operands: bool,
}

impl Rules {
    fn of(engine: Engine) -> Rules {
        Rules {
            levels: if engine < Engine(1, 18, 20) { OLD_LEVELS } else { LEVELS },
            conditionals_chain_left: engine < Engine(1, 18, 10),
            divides_by_magnitude: engine < Engine(1, 19, 60),
            statements_divide_by_magnitude: engine < Engine(1, 20, 50),
            assignments_are_operands: engine < Engine(1, 20, 50),
        }
    }
}

pub(crate) struct Parser<'a> {
    pub(crate) compiler: &'a mut Compiler,
    pub(crate) arrays: Option<&'a Arrays>,
    pub(crate) rules: Rules,
    tokens: Vec<(Token, usize)>,
    at: usize,
    end: usize,
    nodes: Vec<Node>,
    /// Per node, how many nodes deep its tree is: the evaluator's recursion depth.
    heights: Vec<u16>,
    lists: Vec<u32>,
    pub(crate) temps: Vec<String>,
    /// See [`Program::complex`].
    pub(crate) complex: bool,
    /// Divisions to turn into [`Op::DivByMagnitude`] if the program ends up complex.
    pub(crate) statement_divisions: Vec<u32>,
    inlining: u8,
    depth: u8,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(compiler: &'a mut Compiler, arrays: Option<&'a Arrays>) -> Parser<'a> {
        Parser { rules: Rules::of(compiler.engine), compiler, arrays, tokens: Vec::new(), at: 0, end: 0, nodes: Vec::new(), heights: Vec::new(), lists: Vec::new(), temps: Vec::new(), complex: false, statement_divisions: Vec::new(), inlining: 0, depth: 0 }
    }

    pub(crate) fn program(mut self, source: &str) -> Result<Program, Error> {
        self.tokens = tokens(source)?;
        self.end = source.len();
        let (statements, terminated) = self.statements()?;
        if let Some(extra) = self.peek() {
            return Err(self.fail(ErrorKind::UnexpectedToken(extra.written())));
        }
        if self.complex && !terminated {
            return Err(self.fail(ErrorKind::MissingSemicolon));
        }
        let root = match statements[..] {
            [only] if !self.complex => only,
            _ => {
                let list = self.list(&statements);
                self.push(Node::Block(list))
            }
        };
        if self.heights[root as usize] > MAX_HEIGHT {
            return Err(Error { kind: ErrorKind::TooDeep, at: 0 });
        }
        for &division in self.statement_divisions.iter().filter(|_| self.complex) {
            if let Node::Binary(op, ..) = &mut self.nodes[division as usize] {
                *op = Op::DivByMagnitude;
            }
        }
        Ok(Program { nodes: self.nodes.into(), lists: self.lists.into(), root, complex: self.complex, temps: self.temps.len() as u32 })
    }

    /// An array element, compiled into this program.
    pub(crate) fn inline(&mut self, source: &str) -> Result<u32, Error> {
        if self.inlining == 8 {
            return Err(self.fail(ErrorKind::Unsupported("an array that indexes itself")));
        }
        let outer = (std::mem::replace(&mut self.tokens, tokens(source)?), self.at, self.end);
        (self.at, self.end) = (0, source.len());
        self.inlining += 1;
        let node = self.expr().and_then(|node| match self.peek() {
            Some(extra) => Err(self.fail(ErrorKind::UnexpectedToken(extra.written()))),
            None => Ok(node),
        });
        self.inlining -= 1;
        (self.tokens, self.at, self.end) = outer;
        node
    }

    /// Statements up to a `}` or the end, and whether the last one ended in `;`.
    pub(crate) fn statements(&mut self) -> Result<(Vec<u32>, bool), Error> {
        let mut out = Vec::new();
        let mut terminated = false;
        let mut left = false;
        while !matches!(self.peek(), None | Some(Token::Symbol("}"))) {
            if left {
                return Err(self.fail(ErrorKind::Unreachable));
            }
            let node = self.statement()?;
            left = matches!(self.nodes[node as usize], Node::Return(_) | Node::Break | Node::Continue);
            out.push(node);
            terminated = false;
            while self.eat(";") {
                terminated = true;
                self.complex = true;
            }
            if !terminated {
                break;
            }
        }
        if out.is_empty() {
            return Err(self.fail(ErrorKind::UnexpectedEnd));
        }
        Ok((out, terminated))
    }

    pub(crate) fn statement(&mut self) -> Result<u32, Error> {
        let keyword = match self.peek() {
            Some(Token::Name(name)) => name.clone(),
            _ => return self.expr(),
        };
        match keyword.as_str() {
            "return" => {
                self.at += 1;
                let value = self.expr()?;
                Ok(self.push(Node::Return(value)))
            }
            "break" | "continue" => {
                self.at += 1;
                Ok(self.push(if keyword == "break" { Node::Break } else { Node::Continue }))
            }
            _ => self.expr(),
        }
    }

    /// Runs a parser that recurses; the limit keeps hostile nesting from overflowing the stack,
    /// here and in the evaluator.
    pub(crate) fn nested(&mut self, parse: fn(&mut Self) -> Result<u32, Error>) -> Result<u32, Error> {
        if self.depth == 128 {
            return Err(self.fail(ErrorKind::TooDeep));
        }
        self.depth += 1;
        let node = parse(self);
        self.depth -= 1;
        node
    }

    /// An assignment, or anything looser than one.
    pub(crate) fn expr(&mut self) -> Result<u32, Error> {
        self.nested(Self::assignment)
    }

    fn assignment(&mut self) -> Result<u32, Error> {
        if !self.assigns() {
            return self.coalesce();
        }
        let target = self.arrow()?;
        let assignable = match self.nodes[target as usize] {
            Node::Var { .. } | Node::Temp { .. } => true,
            Node::Arrow { of, .. } => matches!(self.nodes[of as usize], Node::Var { .. }),
            _ => false,
        };
        if !assignable {
            return Err(self.fail(ErrorKind::NotAssignable));
        }
        self.at += 1;
        self.complex = true;
        let value = self.coalesce()?;
        Ok(self.push(Node::Assign { target, value }))
    }

    pub(crate) fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at).map(|(token, _)| token)
    }

    /// Whether an assignment comes next: `name =`, or `name->name =` for another entity's variable.
    pub(crate) fn assigns(&self) -> bool {
        let mut at = self.at;
        loop {
            match (self.tokens.get(at), self.tokens.get(at + 1)) {
                (Some((Token::Name(_), _)), Some((Token::Symbol("="), _))) => return true,
                (Some((Token::Name(_), _)), Some((Token::Symbol("->"), _))) => at += 2,
                _ => return false,
            }
        }
    }

    pub(crate) fn next(&mut self) -> Result<Token, Error> {
        let token = self.peek().cloned().ok_or_else(|| self.fail(ErrorKind::UnexpectedEnd))?;
        self.at += 1;
        Ok(token)
    }

    pub(crate) fn eat(&mut self, symbol: &str) -> bool {
        let hit = matches!(self.peek(), Some(Token::Symbol(s)) if *s == symbol);
        self.at += usize::from(hit);
        hit
    }

    pub(crate) fn expect(&mut self, symbol: &str) -> Result<(), Error> {
        if self.eat(symbol) {
            return Ok(());
        }
        Err(self.fail(match self.peek() {
            Some(found) => ErrorKind::UnexpectedToken(found.written()),
            None => ErrorKind::UnexpectedEnd,
        }))
    }

    /// An error at the token about to be read.
    pub(crate) fn fail(&self, kind: ErrorKind) -> Error {
        Error { kind, at: self.tokens.get(self.at).map_or(self.end, |(_, at)| *at) }
    }

    /// An error at the token just read.
    pub(crate) fn fail_last(&self, kind: ErrorKind) -> Error {
        Error { kind, at: self.tokens[self.at - 1].1 }
    }

    pub(crate) fn node(&self, index: u32) -> Node {
        self.nodes[index as usize]
    }

    pub(crate) fn push(&mut self, node: Node) -> u32 {
        let node = match fold(&self.nodes, node) {
            Some(Folded::Node(picked)) => return picked,
            Some(Folded::Value(value)) => Node::Const(value),
            None => node,
        };
        let tallest = |parser: &Self, children: &[u32]| children.iter().map(|&child| parser.heights[child as usize]).max().unwrap_or(0);
        let below = match node {
            Node::Const(_) | Node::This | Node::Var { .. } | Node::Temp { .. } | Node::Context(_) | Node::Break | Node::Continue => 0,
            Node::Not(a) | Node::Neg(a) | Node::Return(a) | Node::Member { of: a, .. } => tallest(self, &[a]),
            Node::Binary(_, a, b) | Node::Coalesce(a, b) | Node::When(a, b) => tallest(self, &[a, b]),
            Node::Assign { target: a, value: b } | Node::Loop { count: a, body: b } | Node::Arrow { entity: a, of: b } => tallest(self, &[a, b]),
            Node::Ternary(a, b, c) | Node::ForEach { variable: a, array: b, body: c } => tallest(self, &[a, b, c]),
            Node::Math { args, count, .. } => tallest(self, &args[..usize::from(count)]),
            Node::Query { args: list, .. } | Node::Block(list) => tallest(self, &self.lists[list.start as usize..][..list.len as usize]),
            Node::Index { items, index } => tallest(self, &self.lists[items.start as usize..][..items.len as usize]).max(self.heights[index as usize]),
        };
        self.heights.push(below + 1);
        self.nodes.push(node);
        self.nodes.len() as u32 - 1
    }

    pub(crate) fn list(&mut self, items: &[u32]) -> List {
        let start = self.lists.len() as u32;
        self.lists.extend_from_slice(items);
        List { start, len: items.len() as u32 }
    }
}
