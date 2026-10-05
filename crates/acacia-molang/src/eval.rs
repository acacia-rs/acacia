//! Evaluation of expressions; statements and `->` are in `exec.rs`. Semantics are BDS's
//! (`tests/oracle/*.bds`); nothing here allocates once the [`Scratch`](crate::Scratch) and
//! [`Variables`](crate::Variables) have grown to fit.
//!
//! Every node comes back in a register: an `f32` from [`Machine::num`], an 8-byte [`Value`] from
//! [`Machine::value`]. `return`, `break` and `continue` therefore set [`Machine::flow`] and are
//! noticed by blocks and loops, not carried in each result.

use crate::compiler::{Context, Query};
use crate::host::{Env, Host, Structs};
use crate::math::MathFn;
use crate::program::{Node, Op, Program};
use crate::store::Store;
use crate::value::{Symbol, Value};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Flow {
    Running,
    Break,
    Continue,
    Return,
}

pub(crate) enum Vars<'a> {
    Own(&'a mut Store),
    /// Another entity's, read through `->`.
    Other(&'a Store),
}

pub(crate) struct Machine<'a> {
    pub(crate) program: &'a Program,
    pub(crate) host: &'a dyn Host,
    pub(crate) vars: Vars<'a>,
    pub(crate) temps: &'a mut Store,
    pub(crate) stack: &'a mut Vec<Value>,
    pub(crate) this: f32,
    /// Loop passes and dice left; see [`REPEATS`].
    pub(crate) repeats: u32,
    pub(crate) flow: Flow,
    /// What a `return` gave, once `flow` is [`Flow::Return`].
    pub(crate) returned: Value,
}

/// How many loop passes and dice one evaluation may spend in total. BDS has no such limit and
/// stalls on a long loop; a pack must not be able to stall whoever evaluates its Molang.
const REPEATS: u32 = 1 << 20;

impl Program {
    pub fn eval(&self, env: &mut Env<'_>) -> Value {
        if let (false, Node::Const(value)) = (self.complex, self.nodes[self.root as usize]) {
            return value;
        }
        env.scratch.temps.reset(self.temps as usize);
        env.scratch.stack.clear();
        let mut machine = Machine {
            program: self,
            host: env.host,
            vars: Vars::Own(&mut env.variables.0),
            temps: &mut env.scratch.temps,
            stack: &mut env.scratch.stack,
            this: env.this,
            repeats: REPEATS,
            flow: Flow::Running,
            returned: Value::ZERO,
        };
        let value = machine.value(self.root);
        match (self.complex, machine.flow) {
            (false, Flow::Running) => value,
            (true, Flow::Return) => machine.returned,
            _ => Value::ZERO,
        }
    }
}

impl Machine<'_> {
    /// The node as a number; anything that is not one counts as 0.
    pub(crate) fn num(&mut self, node: u32) -> f32 {
        match self.program.nodes[node as usize] {
            Node::Const(Value::Num(n)) => n,
            Node::This => self.this,
            Node::Neg(operand) => -self.num(operand),
            Node::Binary(op @ (Op::Add | Op::Sub | Op::Mul | Op::Div | Op::DivByMagnitude), left, right) => {
                let (x, y) = (self.num(left), self.num(right));
                op.arithmetic(x, y)
            }
            Node::Math { function, args, count } => self.math(function, args, count),
            Node::Var { slot, path } => self.variable(slot, self.program.list(path)).map_or(0.0, Value::num),
            Node::Temp { slot, path } => self.temps.get(slot, self.program.list(path)).map_or(0.0, Value::num),
            Node::Query { id, args } => self.query(id, self.program.list(args)).num(),
            Node::Ternary(condition, then, otherwise) => {
                if self.truthy(condition) { self.num(then) } else { self.num(otherwise) }
            }
            _ => self.value(node).num(),
        }
    }

    /// The node as a condition, without making a value of a comparison first.
    pub(crate) fn truthy(&mut self, node: u32) -> bool {
        match self.program.nodes[node as usize] {
            Node::Not(operand) => !self.truthy(operand),
            Node::Binary(Op::Or, left, right) => self.truthy(left) || self.truthy(right),
            Node::Binary(Op::And, left, right) => self.truthy(left) && self.truthy(right),
            Node::Binary(op @ (Op::Lt | Op::Le | Op::Gt | Op::Ge), left, right) => {
                let (x, y) = (self.num(left), self.num(right));
                op.orders(x, y)
            }
            Node::Binary(op @ (Op::Eq | Op::Ne), left, right) => {
                let (a, b) = (self.value(left), self.value(right));
                op.equates(a, b)
            }
            _ => self.value(node).truthy(),
        }
    }

    // Kept out of `num` and `value`, which every node passes through and which must stay small.
    #[inline(never)]
    fn math(&mut self, function: MathFn, args: [u32; 3], count: u8) -> f32 {
        let mut values = [0.0; 3];
        for (value, &arg) in values.iter_mut().zip(&args[..usize::from(count)]) {
            *value = self.num(arg);
        }
        if matches!(function, MathFn::DieRoll | MathFn::DieRollInteger) {
            values[0] = self.spend(values[0]) as f32;
        }
        let host = self.host;
        function.call(values, &|| host.random())
    }

    #[inline(never)]
    fn query(&mut self, id: u32, args: &[u32]) -> Value {
        let base = self.stack.len();
        for &arg in args {
            let value = self.value(arg);
            self.stack.push(value);
        }
        let value = self.host.query(Query(id), &self.stack[base..], &mut Structs(self.temps));
        self.stack.truncate(base);
        value
    }

    /// The node's value; a variable that was never set reads as 0.
    pub(crate) fn value(&mut self, node: u32) -> Value {
        let program = self.program;
        match program.nodes[node as usize] {
            Node::Const(value) => value,
            Node::This | Node::Neg(_) | Node::Math { .. } => Value::Num(self.num(node)),
            Node::Binary(Op::Add | Op::Sub | Op::Mul | Op::Div | Op::DivByMagnitude, ..) => Value::Num(self.num(node)),
            Node::Not(_) | Node::Binary(..) => Value::flag(self.truthy(node)),
            Node::Query { id, args } => self.query(id, program.list(args)),
            Node::Var { slot, path } => self.variable(slot, program.list(path)).unwrap_or(Value::ZERO),
            Node::Temp { slot, path } => self.temps.get(slot, program.list(path)).unwrap_or(Value::ZERO),
            Node::Ternary(condition, then, otherwise) => {
                if self.truthy(condition) { self.value(then) } else { self.value(otherwise) }
            }
            Node::When(condition, then) => {
                if self.truthy(condition) { self.value(then) } else { Value::ZERO }
            }
            Node::Assign { .. } | Node::Block(_) | Node::Loop { .. } | Node::ForEach { .. } | Node::Break | Node::Continue | Node::Return(_) => {
                self.run(node)
            }
            Node::Context(_) | Node::Member { .. } | Node::Coalesce(..) | Node::Index { .. } | Node::Arrow { .. } => {
                self.maybe(node).unwrap_or(Value::ZERO)
            }
        }
    }

    /// The nodes that can be unset, which only `??` tells apart from 0.
    pub(crate) fn maybe(&mut self, node: u32) -> Option<Value> {
        let program = self.program;
        match program.nodes[node as usize] {
            Node::Var { slot, path } => self.variable(slot, program.list(path)),
            Node::Temp { slot, path } => self.temps.get(slot, program.list(path)),
            Node::Context(id) => self.host.context(Context(id)),
            Node::Member { of, path } => {
                let mut value = self.value(of);
                for &member in program.list(path) {
                    let (held, _) = value.storage()?;
                    value = self.store(held).member(value, Symbol(member))?;
                }
                Some(value)
            }
            Node::Coalesce(left, right) => match self.maybe(left) {
                None => self.maybe(right),
                set => set,
            },
            Node::Ternary(condition, then, otherwise) => {
                if self.truthy(condition) { self.maybe(then) } else { self.maybe(otherwise) }
            }
            Node::When(condition, then) => {
                if self.truthy(condition) { self.maybe(then) } else { Some(Value::ZERO) }
            }
            Node::Index { items, index } => {
                let items = program.list(items);
                let index = self.num(index).max(0.0) as usize % items.len();
                self.maybe(items[index])
            }
            Node::Arrow { entity, of } => self.arrow(entity, of),
            _ => Some(self.value(node)),
        }
    }
}
