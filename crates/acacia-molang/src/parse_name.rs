//! Names, calls and the two loops; operators are in `parse_expr.rs`.

use crate::error::{Error, ErrorKind};
use crate::lex::Token;
use crate::math::MathFn;
use crate::parse::Parser;
use crate::program::{List, Node, Op};
use crate::queries;
use crate::value::{Symbol, Value};

impl Parser<'_> {
    pub(crate) fn name(&mut self, name: &str) -> Result<u32, Error> {
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
        if rest.split('.').any(|part| part.starts_with(|c: char| c.is_ascii_digit())) {
            return Err(unknown(self));
        }
        if prefix == "array" {
            return self.index(rest);
        }
        let named = self.fail_last(ErrorKind::NotCallable(name.to_owned()));
        let arguments = if self.eat("(") { Some(self.arguments()?) } else { None };
        let node = match prefix {
            "query" | "q" => {
                // `query.spellcolor.r`: a member of the struct the query returns.
                let (query, members) = rest.split_once('.').unwrap_or((rest, ""));
                if self.compiler.documented_queries_only && !queries::exists(query, self.compiler.engine) {
                    return Err(Error { kind: ErrorKind::UnknownQuery(query.to_owned()), at: named.at });
                }
                let args = self.list(arguments.as_deref().unwrap_or_default());
                let id = self.compiler.queries.intern(query);
                let of = self.push(Node::Query { id, args });
                let path = self.members(members);
                return Ok(if path.len == 0 { of } else { self.push(Node::Member { of, path }) });
            }
            "math" => return self.math(rest, arguments.as_deref().unwrap_or_default(), named.at),
            "variable" | "v" | "temp" | "t" => {
                let (root, members) = rest.split_once('.').unwrap_or((rest, ""));
                let path = self.members(members);
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

    /// The symbols of a dotted member path (`min.x`), as a list.
    fn members(&mut self, dotted: &str) -> List {
        let members: Vec<u32> = dotted.split('.').filter(|m| !m.is_empty()).map(|m| self.compiler.symbol(m).0).collect();
        self.list(&members)
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
        self.numeric(arguments)?;
        // BDS only guards a computed divisor: by a constant 0 the remainder is not a number, whatever
        // the dividend, which still has to be evaluated.
        if matches!(function, MathFn::Mod) && matches!(self.node(arguments[1]), Node::Const(Value::Num(n)) if n == 0.0) {
            let nan = self.push(Node::Const(Value::Num(f32::NAN)));
            return Ok(self.push(Node::Binary(Op::Mul, arguments[0], nan)));
        }
        let mut args = [0; 3];
        args[..arguments.len()].copy_from_slice(arguments);
        if let Some(moved) = self.moved_argument(function, arguments) {
            args[moved] = self.moved(args[moved]);
        }
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
}
