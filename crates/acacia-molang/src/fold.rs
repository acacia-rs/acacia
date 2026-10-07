//! Constant folding: a node whose operands are all literals is computed while compiling, with the
//! evaluator's own arithmetic ([`Op`]'s methods, [`MathFn::call`]).

use crate::math::MathFn;
use crate::program::{Node, Op};
use crate::value::Value;

pub(crate) enum Folded {
    Value(Value),
    /// A conditional on a literal is the branch it picks.
    Node(u32),
}

pub(crate) fn fold(nodes: &[Node], node: Node) -> Option<Folded> {
    // A literal's own value: BDS's folding ignores a post-op left on it (`optimise.rs`).
    let literal = |index: u32| match nodes[index as usize] {
        Node::Const(value) => Some(value),
        Node::Post { of, .. } => match nodes[of as usize] {
            Node::Const(value) => Some(value),
            _ => None,
        },
        _ => None,
    };
    // A `return`, `break` or `continue` stays under its conditional, where the parser looks for it.
    // So does a string: folded, `1 + (1 ? 'a' : 2)` would read as the `1 + 'a'` BDS rejects.
    let branch = |index: u32| match nodes[index as usize] {
        Node::Return(_) | Node::Break | Node::Continue | Node::Const(Value::Str(_)) => None,
        _ => Some(Folded::Node(index)),
    };
    Some(Folded::Value(match node {
        Node::Not(operand) => Value::flag(!literal(operand)?.truthy()),
        Node::Binary(op, left, right) => {
            let (a, b) = (literal(left)?, literal(right)?);
            match op {
                Op::Add | Op::Sub | Op::Mul | Op::Div | Op::DivByMagnitude => Value::Num(op.arithmetic(a.num(), b.num())),
                // Built as `Node::Logic`.
                Op::Or | Op::And => return None,
                Op::Eq | Op::Ne => Value::flag(op.equates(a, b)),
                Op::Lt | Op::Le | Op::Gt | Op::Ge => Value::flag(op.orders(a.num(), b.num())),
            }
        }
        Node::Math { function, args, count } if !function.is_random() => {
            let mut values = [0.0; 3];
            for (value, &arg) in values.iter_mut().zip(&args[..usize::from(count)]) {
                *value = literal(arg)?.num();
            }
            Value::Num(MathFn::call(function, values, &|| 0.0))
        }
        Node::Ternary(condition, then, otherwise) => return branch(if literal(condition)?.truthy() { then } else { otherwise }),
        Node::When(condition, then) if literal(condition)?.truthy() => return branch(then),
        Node::When(..) => Value::ZERO,
        _ => return None,
    }))
}
