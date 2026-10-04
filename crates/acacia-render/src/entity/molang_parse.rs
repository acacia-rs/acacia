//! Recursive-descent parser for [`super::molang`]. Input is already lowercase.

use super::molang::{Expr, Op, Program, Value};

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(f32),
    Text(String),
    Name(String),
    Symbol(&'static str),
}

/// Longest first, so `==` is not read as two `=`.
const SYMBOLS: [&str; 22] =
    ["&&", "||", "==", "!=", "<=", ">=", "??", "(", ")", "[", "]", ",", ";", "?", ":", "!", "<", ">", "+", "-", "*", "/"];

fn tokens(source: &str) -> Option<Vec<Token>> {
    let mut out = Vec::new();
    let mut rest = source.trim_start();
    while let Some(first) = rest.chars().next() {
        let take = |pred: fn(char) -> bool| rest.find(|c| !pred(c)).unwrap_or(rest.len());
        let len = if first.is_ascii_digit() || (first == '.' && rest[1..].starts_with(|c: char| c.is_ascii_digit())) {
            let len = take(|c| c.is_ascii_digit() || c == '.');
            out.push(Token::Num(rest[..len].parse().ok()?));
            // C-style float suffix: `1.0f`.
            len + usize::from(rest[len..].starts_with('f') && !rest[len + 1..].starts_with(|c: char| c.is_ascii_alphanumeric()))
        } else if first.is_ascii_alphabetic() || first == '_' {
            let len = take(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
            out.push(Token::Name(rest[..len].to_owned()));
            len
        } else if first == '\'' {
            let end = rest[1..].find('\'')?;
            out.push(Token::Text(rest[1..=end].to_owned()));
            end + 2
        } else if let Some(symbol) = SYMBOLS.iter().find(|s| rest.starts_with(**s)) {
            out.push(Token::Symbol(symbol));
            symbol.len()
        } else if first == '=' {
            out.push(Token::Symbol("="));
            1
        } else {
            return None;
        };
        rest = rest[len..].trim_start();
    }
    Some(out)
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

/// Binary operators from loosest to tightest.
const LEVELS: [&[(&str, Op)]; 7] = [
    &[("??", Op::Coalesce)],
    &[("||", Op::Or)],
    &[("&&", Op::And)],
    &[("==", Op::Eq), ("!=", Op::Ne)],
    &[("<=", Op::Le), (">=", Op::Ge), ("<", Op::Lt), (">", Op::Gt)],
    &[("+", Op::Add), ("-", Op::Sub)],
    &[("*", Op::Mul), ("/", Op::Div)],
];

impl Parser {
    fn eat(&mut self, symbol: &str) -> bool {
        let hit = matches!(self.tokens.get(self.at), Some(Token::Symbol(s)) if *s == symbol);
        self.at += usize::from(hit);
        hit
    }

    fn statement(&mut self) -> Option<Expr> {
        if let (Some(Token::Name(name)), Some(Token::Symbol("="))) = (self.tokens.get(self.at), self.tokens.get(self.at + 1)) {
            let name = name.clone();
            self.at += 2;
            return Some(Expr::Assign(name, Box::new(self.ternary()?)));
        }
        if matches!(self.tokens.get(self.at), Some(Token::Name(n)) if n == "return") {
            self.at += 1;
        }
        self.ternary()
    }

    fn ternary(&mut self) -> Option<Expr> {
        let condition = self.binary(0)?;
        if !self.eat("?") {
            return Some(condition);
        }
        let then = self.ternary()?;
        // `a ? b` without an else yields 0 otherwise.
        let otherwise = if self.eat(":") { self.ternary()? } else { Expr::Value(Value::Num(0.0)) };
        Some(Expr::Ternary(Box::new(condition), Box::new(then), Box::new(otherwise)))
    }

    fn binary(&mut self, level: usize) -> Option<Expr> {
        let Some(ops) = LEVELS.get(level) else { return self.unary() };
        let mut left = self.binary(level + 1)?;
        while let Some(&(_, op)) = ops.iter().find(|(symbol, _)| self.eat(symbol)) {
            left = Expr::Binary(op, Box::new(left), Box::new(self.binary(level + 1)?));
        }
        Some(left)
    }

    fn unary(&mut self) -> Option<Expr> {
        if self.eat("!") {
            return Some(Expr::Not(Box::new(self.unary()?)));
        }
        if self.eat("-") {
            return Some(Expr::Neg(Box::new(self.unary()?)));
        }
        let token = self.tokens.get(self.at)?.clone();
        self.at += 1;
        match token {
            Token::Num(n) => Some(Expr::Value(Value::Num(n))),
            Token::Text(t) => Some(Expr::Value(Value::Text(t))),
            Token::Symbol("(") => {
                let inner = self.ternary()?;
                self.eat(")").then_some(inner)
            }
            Token::Symbol(_) => None,
            Token::Name(name) if name == "this" => Some(Expr::This),
            Token::Name(name) if name == "true" || name == "false" => Some(Expr::Value(Value::Num(f32::from(u8::from(name == "true"))))),
            Token::Name(name) => {
                if self.eat("[") {
                    let index = self.ternary()?;
                    return self.eat("]").then(|| Expr::Index(name, Box::new(index)));
                }
                let mut args = Vec::new();
                if self.eat("(") {
                    while !self.eat(")") {
                        args.push(self.ternary()?);
                        self.eat(",");
                    }
                }
                Some(Expr::Name(name, args))
            }
        }
    }
}

pub fn parse_program(source: &str) -> Option<Program> {
    let mut parser = Parser { tokens: tokens(source)?, at: 0 };
    let mut statements = Vec::new();
    while parser.at < parser.tokens.len() {
        statements.push(parser.statement()?);
        if !parser.eat(";") && parser.at < parser.tokens.len() {
            return None;
        }
    }
    Some(Program(statements))
}
