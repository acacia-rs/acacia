//! Operators, loosest first, then values and names. See `parse.rs`.

use crate::error::{Error, ErrorKind};
use crate::lex::Token;
use crate::math::MathFn;
use crate::parse::Parser;
use crate::program::{Node, Op};
use crate::value::{Symbol, Value};

pub(crate) type Levels = &'static [&'static [(&'static str, Op)]];

/// Binary operators from loosest to tightest; each level is left-associative.
pub(crate) const LEVELS: Levels = &[
    &[("||", Op::Or)],
    &[("&&", Op::And)],
    &[("==", Op::Eq), ("!=", Op::Ne)],
    &[("<=", Op::Le), (">=", Op::Ge), ("<", Op::Lt), (">", Op::Gt)],
    &[("+", Op::Add), ("-", Op::Sub)],
    &[("*", Op::Mul), ("/", Op::Div)],
];

/// The same for packs older than 1.18.20, where every comparison and logic operator had a rank of
/// its own, `||` above `&&` (`tests/oracle/versions@1.13.0.bds`).
pub(crate) const OLD_LEVELS: Levels = &[
    &[("&&", Op::And)],
    &[("||", Op::Or)],
    &[("!=", Op::Ne)],
    &[("<=", Op::Le)],
    &[(">", Op::Gt)],
    &[(">=", Op::Ge)],
    &[("==", Op::Eq)],
    &[("<", Op::Lt)],
    &[("+", Op::Add), ("-", Op::Sub)],
    &[("*", Op::Mul), ("/", Op::Div)],
];

impl Parser<'_> {
    /// `??` is the loosest operator and left-associative.
    pub(crate) fn coalesce(&mut self) -> Result<u32, Error> {
        let mut left = self.ternary()?;
        while self.eat("??") {
            // BDS only promises `??` for plain variables; a member on the left it rejects outright.
            if matches!(self.node(left), Node::Var { path, .. } | Node::Temp { path, .. } if path.len > 0) {
                return Err(self.fail_last(ErrorKind::CoalesceTarget));
            }
            let right = self.ternary()?;
            left = self.push(Node::Coalesce(left, right));
        }
        Ok(left)
    }

    fn ternary(&mut self) -> Result<u32, Error> {
        let mut condition = self.binary(0)?;
        while self.eat("?") {
            let then = self.branch()?;
            condition = if self.eat(":") {
                let otherwise = self.branch()?;
                self.push(Node::Ternary(condition, then, otherwise))
            } else {
                self.push(Node::When(condition, then))
            };
            // Only old packs chain to the left: `a ? b : c ? d : e` is `(a ? b : c) ? d : e` there.
            if !self.rules.conditionals_chain_left {
                break;
            }
        }
        Ok(condition)
    }

    /// One side of a conditional: also `break`, `continue`, `return x` and assignments.
    fn branch(&mut self) -> Result<u32, Error> {
        match (self.peek(), self.peek_after()) {
            (Some(Token::Name(name)), _) if matches!(name.as_str(), "return" | "break" | "continue") => self.statement(),
            (Some(Token::Name(_)), Some(Token::Symbol("="))) => self.expr(),
            _ if self.rules.conditionals_chain_left => self.binary(0),
            _ => self.nested(Self::ternary),
        }
    }

    fn binary(&mut self, level: usize) -> Result<u32, Error> {
        let Some(ops) = self.rules.levels.get(level) else { return self.unary() };
        let mut left = self.binary(level + 1)?;
        while let Some(&(_, op)) = ops.iter().find(|(symbol, _)| self.eat(symbol)) {
            let right = self.binary(level + 1)?;
            let text = |node| matches!(node, Node::Const(Value::Str(_)));
            if !matches!(op, Op::Eq | Op::Ne) && (text(self.node(left)) || text(self.node(right))) {
                return Err(self.fail_last(ErrorKind::StringOperand));
            }
            let old_division = op == Op::Div && !self.folds(right);
            let op = if old_division && self.rules.divides_by_magnitude { Op::DivByMagnitude } else { op };
            left = self.push(Node::Binary(op, left, right));
            if old_division && self.rules.statements_divide_by_magnitude {
                self.statement_divisions.push(left);
            }
        }
        Ok(left)
    }

    /// Whether BDS computes the node while loading: operators over literals, as in `0 - 2`.
    fn folds(&self, node: u32) -> bool {
        match self.node(node) {
            Node::Const(_) => true,
            Node::Not(operand) | Node::Neg(operand) => self.folds(operand),
            Node::Binary(_, left, right) => self.folds(left) && self.folds(right),
            _ => false,
        }
    }

    fn unary(&mut self) -> Result<u32, Error> {
        self.nested(Self::prefixed)
    }

    fn prefixed(&mut self) -> Result<u32, Error> {
        if self.eat("!") {
            let operand = self.unary()?;
            return Ok(self.push(Node::Not(operand)));
        }
        if !self.eat("-") {
            return self.arrow();
        }
        if matches!(self.peek(), Some(Token::Symbol("-"))) {
            return Err(self.fail(ErrorKind::DoubleNegation));
        }
        let operand = self.unary()?;
        Ok(match self.node(operand) {
            Node::Const(Value::Num(n)) => self.push(Node::Const(Value::Num(-n))),
            _ => self.push(Node::Neg(operand)),
        })
    }

    /// `entity->expression`
    fn arrow(&mut self) -> Result<u32, Error> {
        let mut left = self.primary()?;
        while self.eat("->") {
            let of = self.primary()?;
            left = self.push(Node::Arrow { entity: left, of });
        }
        Ok(left)
    }

    pub(crate) fn primary(&mut self) -> Result<u32, Error> {
        match self.next()? {
            Token::Num(n) => Ok(self.push(Node::Const(Value::Num(n)))),
            Token::Text(text) => {
                let symbol = self.compiler.symbol(&text);
                Ok(self.push(Node::Const(Value::Str(symbol))))
            }
            Token::Symbol("(") => {
                let inner = self.expr()?;
                self.expect(")")?;
                Ok(inner)
            }
            Token::Symbol("{") => {
                let (statements, _) = self.statements()?;
                self.expect("}")?;
                let list = self.list(&statements);
                Ok(self.push(Node::Block(list)))
            }
            Token::Symbol(symbol) => Err(self.fail_last(ErrorKind::UnexpectedToken(symbol.to_owned()))),
            Token::Name(name) => self.name(&name),
        }
    }

    fn name(&mut self, name: &str) -> Result<u32, Error> {
        match name {
            "this" => return Ok(self.push(Node::This)),
            "true" | "false" => return Ok(self.push(Node::Const(Value::flag(name == "true")))),
            "loop" => return self.repeat(),
            "for_each" => return self.for_each(),
            "return" | "break" | "continue" => return Err(self.fail_last(ErrorKind::UnexpectedToken(name.to_owned()))),
            _ => {}
        }
        let unknown = |parser: &Parser| parser.fail_last(ErrorKind::UnknownName(name.to_owned()));
        let Some((prefix, rest)) = name.split_once('.').filter(|(_, rest)| !rest.is_empty()) else { return Err(unknown(self)) };
        if prefix == "array" {
            return self.index(rest);
        }
        let named = self.fail_last(ErrorKind::NotCallable(name.to_owned()));
        let arguments = if self.eat("(") { Some(self.arguments()?) } else { None };
        let node = match prefix {
            "query" | "q" => {
                let args = self.list(arguments.as_deref().unwrap_or_default());
                let id = self.compiler.queries.intern(rest);
                return Ok(self.push(Node::Query { id, args }));
            }
            "math" => return self.math(rest, arguments.as_deref().unwrap_or_default(), named.at),
            "variable" | "v" | "temp" | "t" => {
                let (root, members) = rest.split_once('.').unwrap_or((rest, ""));
                let members: Vec<u32> = members.split('.').filter(|m| !m.is_empty()).map(|m| self.compiler.symbol(m).0).collect();
                let path = self.list(&members);
                if prefix.starts_with('v') {
                    Node::Var { slot: self.compiler.variables.intern(root), path }
                } else {
                    let known = self.temps.iter().position(|t| t == root);
                    let slot = known.unwrap_or_else(|| {
                        self.temps.push(root.to_owned());
                        self.temps.len() - 1
                    });
                    Node::Temp { slot: slot as u32, path }
                }
            }
            "context" | "c" => Node::Context(self.compiler.contexts.intern(rest)),
            "geometry" | "texture" | "material" => Node::Const(Value::Str(Symbol(self.compiler.strings.intern(name)))),
            _ => return Err(unknown(self)),
        };
        if arguments.is_some() {
            return Err(named);
        }
        Ok(self.push(node))
    }

    fn arguments(&mut self) -> Result<Vec<u32>, Error> {
        let mut out = Vec::new();
        if self.eat(")") {
            return Ok(out);
        }
        loop {
            out.push(self.expr()?);
            if !self.eat(",") {
                self.expect(")")?;
                return Ok(out);
            }
        }
    }

    fn math(&mut self, name: &str, arguments: &[u32], at: usize) -> Result<u32, Error> {
        let Some((function, name, expected)) = MathFn::find(name) else {
            return Err(Error { kind: ErrorKind::UnknownMath(name.to_owned()), at });
        };
        if arguments.len() != usize::from(expected) {
            return Err(Error { kind: ErrorKind::ArgumentCount { name, expected, found: arguments.len() }, at });
        }
        let mut args = [0; 3];
        args[..arguments.len()].copy_from_slice(arguments);
        Ok(self.push(Node::Math { function, args, count: expected }))
    }

    /// `loop(count, { body })`
    fn repeat(&mut self) -> Result<u32, Error> {
        self.expect("(")?;
        let count = self.expr()?;
        let body = self.body()?;
        Ok(self.push(Node::Loop { count, body }))
    }

    /// `for_each(variable, array, { body })`
    fn for_each(&mut self) -> Result<u32, Error> {
        self.expect("(")?;
        let variable = self.primary()?;
        if !matches!(self.node(variable), Node::Var { .. } | Node::Temp { .. }) {
            return Err(self.fail_last(ErrorKind::NotAssignable));
        }
        self.expect(",")?;
        let array = self.expr()?;
        let body = self.body()?;
        Ok(self.push(Node::ForEach { variable, array, body }))
    }

    /// `, { statements })`: the last argument of both loops.
    fn body(&mut self) -> Result<u32, Error> {
        self.expect(",")?;
        if !matches!(self.peek(), Some(Token::Symbol("{"))) {
            return Err(self.fail(ErrorKind::LoopBody));
        }
        let body = self.primary()?;
        self.expect(")")?;
        Ok(body)
    }

    /// `array.name[index]`
    fn index(&mut self, array: &str) -> Result<u32, Error> {
        let unknown = self.fail_last(ErrorKind::UnknownArray(array.to_owned()));
        self.expect("[")?;
        let index = self.expr()?;
        self.expect("]")?;
        let mut items = Vec::new();
        self.elements(array, &unknown, 0, &mut items)?;
        if items.is_empty() {
            return Err(unknown);
        }
        let items = self.list(&items);
        Ok(self.push(Node::Index { items, index }))
    }

    /// Compiles an array's elements into `out`. An element that names an array stands for all of
    /// that array's elements.
    fn elements(&mut self, array: &str, unknown: &Error, depth: u8, out: &mut Vec<u32>) -> Result<(), Error> {
        let sources = self.arrays.and_then(|arrays| arrays.get(array)).filter(|_| depth < 8).ok_or_else(|| unknown.clone())?;
        for source in sources {
            let lower = source.trim().to_ascii_lowercase();
            match lower.strip_prefix("array.").filter(|name| !name.contains('[')) {
                Some(included) => self.elements(included, unknown, depth + 1, out)?,
                None => out.push(self.inline(source)?),
            }
        }
        Ok(())
    }
}
