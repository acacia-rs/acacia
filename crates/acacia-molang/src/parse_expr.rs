//! Operators, loosest first, then literals and brackets; names are in `parse_name.rs`. See `parse.rs`.

use crate::error::{Error, ErrorKind};
use crate::lex::Token;
use crate::parse::Parser;
use crate::program::{Node, Op};
use crate::value::Value;

pub(crate) type Levels = &'static [&'static [(&'static str, Op)]];

/// Binary operators from loosest to tightest; each level is left-associative. `/` binds tighter than
/// `*`, as in BDS: `a * b / c` is `a * (b / c)`.
pub(crate) const LEVELS: Levels = &[
    &[("||", Op::Or)],
    &[("&&", Op::And)],
    &[("==", Op::Eq), ("!=", Op::Ne)],
    &[("<=", Op::Le), (">=", Op::Ge), ("<", Op::Lt), (">", Op::Gt)],
    &[("+", Op::Add), ("-", Op::Sub)],
    &[("*", Op::Mul)],
    &[("/", Op::Div)],
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
    &[("*", Op::Mul)],
    &[("/", Op::Div)],
];

impl Parser<'_> {
    /// `??` is the loosest operator and left-associative.
    pub(crate) fn coalesce(&mut self) -> Result<u32, Error> {
        let mut left = self.ternary()?;
        while self.eat("??") {
            // BDS rejects a literal, a call, a conditional or another `??` on the left. A query it
            // evaluates, logging "isn't a direct-variable reference"; `->` it could not be asked about.
            let direct = match self.node(left) {
                Node::Var { path, .. } | Node::Temp { path, .. } => path.len == 0,
                Node::Context(_) | Node::Query { .. } | Node::Member { .. } | Node::Arrow { .. } => true,
                _ => false,
            };
            if !direct {
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
        match self.peek() {
            Some(Token::Name(name)) if matches!(name.as_str(), "return" | "break" | "continue") => self.statement(),
            _ if self.assigns() => self.expr(),
            _ if self.rules.conditionals_chain_left => self.binary(0),
            _ => self.nested(Self::ternary),
        }
    }

    fn binary(&mut self, level: usize) -> Result<u32, Error> {
        let Some(ops) = self.rules.levels.get(level) else { return self.unary() };
        let mut left = self.binary(level + 1)?;
        while let Some(&(_, op)) = ops.iter().find(|(symbol, _)| self.eat(symbol)) {
            let right = self.binary(level + 1)?;
            if !matches!(op, Op::Eq | Op::Ne) {
                self.numeric(&[left, right])?;
            }
            left = match op {
                Op::Add => self.sum(left, right),
                Op::Sub => {
                    let negated = self.negate(right);
                    self.sum(left, negated)
                }
                Op::Mul => self.product(left, right),
                Op::Div => self.quotient(left, right),
                Op::And | Op::Or => self.logic(op == Op::Or, left, right),
                _ => match self.moved_operand(op, left, right) {
                    Some(moved) if moved == left => {
                        let left = self.moved(left);
                        self.push(Node::Binary(op, left, right))
                    }
                    Some(_) => {
                        let right = self.moved(right);
                        self.push(Node::Binary(op, left, right))
                    }
                    None => self.push(Node::Binary(op, left, right)),
                },
            };
        }
        Ok(left)
    }

    /// BDS folds constants too (`0 - 2`, `math.min(-2, 0)`), and a folded divisor keeps its sign.
    fn quotient(&mut self, left: u32, right: u32) -> u32 {
        match (self.raw(left), self.raw(right)) {
            (Some(a), Some(b)) => self.push(Node::Const(Value::Num(Op::Div.arithmetic(a, b)))),
            // BDS multiplies by the reciprocal of a constant divisor, 0 for one below `f32::EPSILON`:
            // the last bit differs from a division, and a not-a-number over 0 stays one.
            (None, Some(divisor)) => {
                let reciprocal = self.push(Node::Const(Value::Num(if divisor.abs() >= f32::EPSILON { 1.0 / divisor } else { 0.0 })));
                self.push(Node::Binary(Op::Mul, left, reciprocal))
            }
            _ => {
                let op = if self.rules.divides_by_magnitude { Op::DivByMagnitude } else { Op::Div };
                let division = self.push(Node::Binary(op, left, right));
                if self.rules.statements_divide_by_magnitude {
                    self.statement_divisions.push(division);
                }
                division
            }
        }
    }

    /// Operands of an operator or function that only takes numbers.
    pub(crate) fn numeric(&self, operands: &[u32]) -> Result<(), Error> {
        for &operand in operands {
            match self.node(operand) {
                Node::Const(Value::Str(_)) => return Err(self.fail_last(ErrorKind::StringOperand)),
                Node::Assign { .. } if !self.rules.assignments_are_operands => return Err(self.fail_last(ErrorKind::AssignmentOperand)),
                _ => {}
            }
        }
        Ok(())
    }

    fn unary(&mut self) -> Result<u32, Error> {
        self.nested(Self::prefixed)
    }

    fn prefixed(&mut self) -> Result<u32, Error> {
        if self.eat("!") {
            let operand = self.unary()?;
            self.numeric(&[operand])?;
            return Ok(self.push(Node::Not(operand)));
        }
        if !self.eat("-") {
            return self.arrow();
        }
        if matches!(self.peek(), Some(Token::Symbol("-"))) {
            return Err(self.fail(ErrorKind::DoubleNegation));
        }
        let operand = self.unary()?;
        self.numeric(&[operand])?;
        Ok(self.negate(operand))
    }

    /// `entity->expression`
    pub(crate) fn arrow(&mut self) -> Result<u32, Error> {
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
                let (statements, terminated) = self.statements()?;
                if !terminated && !statements.is_empty() {
                    return Err(self.fail(ErrorKind::MissingSemicolon));
                }
                self.expect("}")?;
                let list = self.list(&statements);
                Ok(self.push(Node::Block(list)))
            }
            Token::Symbol(symbol) => Err(self.fail_last(ErrorKind::UnexpectedToken(symbol.to_owned()))),
            Token::Name(name) => self.name(&name),
        }
    }
}
