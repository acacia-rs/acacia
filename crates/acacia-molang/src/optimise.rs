//! BDS's optimiser, applied as nodes are built: `-x` and `x·c` fold into post-ops (`post.rs`), `+`
//! and `-` flatten into one sum whose terms merge (`sum.rs`), and `&&`/`||` chains become one node. The merges
//! keep BDS's own results, odd ones included: `v.x - v.x` reads nothing, a merged term loses its
//! offset, and a sum whose terms all cancel counts its constant twice (`tests/oracle/sweeps.bds`).
//! molangx (`sema/sum.rs`, `sema/fold.rs`) describes the same rules.

use crate::math::MathFn;
use crate::parse::Parser;
use crate::post::Post;
use crate::program::{Node, Op};
use crate::value::Value;

type Term = (u32, Post);

impl Parser<'_> {
    pub(crate) fn split(&self, node: u32) -> Term {
        match self.node(node) {
            Node::Post { of, post } => (of, post),
            _ => (node, Post::IDENTITY),
        }
    }

    pub(crate) fn wrap(&mut self, (of, post): Term) -> u32 {
        if post.is_identity() { of } else { self.push(Node::Post { of, post }) }
    }

    /// A number literal's own value, as BDS's folding reads it: without its post-op.
    pub(crate) fn raw(&self, node: u32) -> Option<f32> {
        match self.node(self.split(node).0) {
            Node::Const(Value::Num(n)) => Some(n),
            _ => None,
        }
    }

    /// A number literal moved into its parent: through its post-op, even the identity's `n·1 + 0`.
    pub(crate) fn absorbed(&self, node: u32) -> Option<f32> {
        let post = self.split(node).1;
        self.raw(node).map(|n| n * post.scale + post.offset)
    }

    /// The literal BDS moves into a comparison: the second operand of `<`, `<=`, `>`, `>=`, either of
    /// `==` and `!=` (a string when neither is a number).
    pub(crate) fn moved_operand(&self, op: Op, left: u32, right: u32) -> Option<u32> {
        let literal = |node: u32| self.raw(node).is_some();
        let string = |node: u32| matches!(self.node(node), Node::Const(Value::Str(_)));
        if literal(left) && literal(right) {
            return None;
        }
        match op {
            Op::Lt | Op::Le | Op::Gt | Op::Ge if literal(right) => Some(right),
            Op::Eq | Op::Ne if literal(left) || (!literal(right) && string(left)) => Some(left),
            Op::Eq | Op::Ne if literal(right) || string(right) => Some(right),
            _ => None,
        }
    }

    /// The same for `math.min` and `math.max` (either argument) and `math.pow` and `math.mod` (the second).
    pub(crate) fn moved_argument(&self, function: MathFn, args: &[u32]) -> Option<usize> {
        if args.iter().all(|&arg| self.raw(arg).is_some()) {
            return None;
        }
        match (function, args) {
            (MathFn::Min | MathFn::Max, &[a, _]) if self.raw(a).is_some() => Some(0),
            (MathFn::Min | MathFn::Max | MathFn::Pow | MathFn::Mod, &[_, b]) if self.raw(b).is_some() => Some(1),
            _ => None,
        }
    }

    /// A moved literal counts through its post-op.
    pub(crate) fn moved(&mut self, node: u32) -> u32 {
        match self.absorbed(node) {
            Some(n) => self.number(n),
            None => node,
        }
    }

    pub(crate) fn number(&mut self, n: f32) -> u32 {
        self.push(Node::Const(Value::Num(n)))
    }

    /// A node folded to a number keeps its own post-op, which BDS then applies again.
    pub(crate) fn number_with(&mut self, offset: f32) -> u32 {
        let number = self.number(offset);
        self.wrap((number, Post { scale: 1.0, offset }))
    }

    pub(crate) fn negate(&mut self, x: u32) -> u32 {
        if let Some(n) = self.raw(x) {
            return self.number(-n);
        }
        let (of, post) = self.split(x);
        self.wrap((of, post.negated()))
    }

    pub(crate) fn product(&mut self, left: u32, right: u32) -> u32 {
        if let (Some(a), Some(b)) = (self.raw(left), self.raw(right)) {
            return self.number(a * b);
        }
        let (scaled, factor) = match (self.absorbed(left), self.absorbed(right)) {
            (Some(factor), _) => (right, factor),
            (_, Some(factor)) => (left, factor),
            _ => return self.push(Node::Binary(Op::Mul, left, right)),
        };
        let (of, post) = self.split(scaled);
        self.wrap((of, post.scaled(factor)))
    }

    pub(crate) fn logic(&mut self, any: bool, left: u32, right: u32) -> u32 {
        if let (Some(a), Some(b)) = (self.raw(left), self.raw(right)) {
            let result = if any { a != 0.0 || b != 0.0 } else { a != 0.0 && b != 0.0 };
            return self.push(Node::Const(Value::flag(result)));
        }
        let mut terms = Vec::new();
        for child in [left, right] {
            // A nested chain of the same operator joins this one, its post-op dropped.
            match self.node(self.split(child).0) {
                Node::Logic { any: same, terms: list } if same == any => terms.extend_from_slice(self.items(list)),
                _ => terms.push(child),
            }
        }
        let terms = self.list(&terms);
        self.push(Node::Logic { any, terms })
    }
}
