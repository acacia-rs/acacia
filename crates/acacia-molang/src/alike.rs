//! Whether the terms of a sum print alike, BDS's test for merging them (`optimise.rs`). BDS compares
//! printed text: names, operators, numbers to six decimals and post-ops, but not the literal it moved
//! into a comparison or a `math.min`, so `(v.x == 1) + (v.x == 2)` is `(v.x == 1) * 2`. A query, an
//! assignment or a random function makes a term unlike everything.

use crate::math::MathFn;
use crate::parse::Parser;
use crate::sum::Summand;
use crate::post::{Post, print_alike};
use crate::program::{Node, Op};
use crate::value::Value;

fn posts_alike(a: Post, b: Post) -> bool {
    match (a.is_identity(), b.is_identity()) {
        (true, true) => true,
        (false, false) => print_alike(a.scale, b.scale) && print_alike(a.offset, b.offset),
        _ => false,
    }
}

impl Parser<'_> {
    pub(crate) fn terms_alike(&self, terms: &[Summand]) -> bool {
        let Some((first, rest)) = terms.split_first() else { return true };
        rest.iter().all(|term| self.alike(first.base, term.base) && posts_alike(first.post, term.post))
    }

    fn alike(&self, a: u32, b: u32) -> bool {
        match (self.node(a), self.node(b)) {
            (Node::Post { of: x, post: p }, Node::Post { of: y, post: q }) => posts_alike(p, q) && self.alike(x, y),
            (Node::Const(Value::Num(x)), Node::Const(Value::Num(y))) => print_alike(x, y),
            (Node::Const(x), Node::Const(y)) => x == y,
            (Node::This, Node::This) => true,
            (Node::Var { slot: s, path: p }, Node::Var { slot: t, path: q }) | (Node::Temp { slot: s, path: p }, Node::Temp { slot: t, path: q }) => {
                s == t && self.items(p) == self.items(q)
            }
            (Node::Context(x), Node::Context(y)) => x == y,
            (Node::Not(x), Node::Not(y)) => self.alike(x, y),
            (Node::Math { function: f, args: x, count }, Node::Math { function: g, args: y, .. }) if f == g && !f.is_random() => {
                let count = usize::from(count);
                self.all_alike(&kept_math(self, f, &x[..count]), &kept_math(self, g, &y[..count]))
            }
            (Node::Binary(o, l, r), Node::Binary(p, m, s)) if o == p => self.all_alike(&kept_binary(self, o, l, r), &kept_binary(self, p, m, s)),
            (Node::Sum(x), Node::Sum(y)) => self.all_alike(self.items(x), self.items(y)),
            (Node::Logic { any: a, terms: x }, Node::Logic { any: b, terms: y }) if a == b => self.all_alike(self.items(x), self.items(y)),
            (Node::Ternary(a, b, c), Node::Ternary(d, e, f)) => self.all_alike(&[a, b, c], &[d, e, f]),
            (Node::When(a, b), Node::When(c, d)) | (Node::Coalesce(a, b), Node::Coalesce(c, d)) => self.all_alike(&[a, b], &[c, d]),
            _ => false,
        }
    }

    fn all_alike(&self, a: &[u32], b: &[u32]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| self.alike(x, y))
    }
}

fn kept_binary(parser: &Parser, op: Op, left: u32, right: u32) -> Vec<u32> {
    let moved = parser.moved_operand(op, left, right);
    [left, right].into_iter().filter(|&operand| Some(operand) != moved).collect()
}

fn kept_math(parser: &Parser, function: MathFn, args: &[u32]) -> Vec<u32> {
    let moved = parser.moved_argument(function, args);
    args.iter().enumerate().filter(|&(index, _)| Some(index) != moved).map(|(_, &arg)| arg).collect()
}
