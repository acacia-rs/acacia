//! Evaluation. Semantics are BDS's (`tests/oracle/*.bds`); nothing here allocates once the
//! [`Scratch`](crate::Scratch) and [`Variables`](crate::Variables) have grown to fit.

use crate::compiler::{Context, Query};
use crate::host::{Env, Host, Structs};
use crate::math::MathFn;
use crate::program::{Node, Op, Program};
use crate::store::Store;
use crate::value::Value;

enum Flow {
    Break,
    Continue,
    Return(Value),
}

/// `None` is a variable that was never set, which only `??` tells apart from 0.
type Step = Result<Option<Value>, Flow>;

enum Vars<'a> {
    Own(&'a mut Store),
    /// Another entity's, read through `->`.
    Other(&'a Store),
}

struct Machine<'a> {
    program: &'a Program,
    host: &'a dyn Host,
    vars: Vars<'a>,
    temps: &'a mut Store,
    stack: &'a mut Vec<Value>,
    this: f32,
    /// Loop passes and dice left; see [`REPEATS`].
    repeats: u32,
}

/// How many loop passes and dice one evaluation may spend in total. BDS has no such limit and
/// stalls on a long loop; a pack must not be able to stall whoever evaluates its Molang.
const REPEATS: u32 = 1 << 20;

impl Program {
    pub fn eval(&self, env: &mut Env<'_>) -> Value {
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
        };
        match (self.complex, machine.step(self.root)) {
            (false, Ok(value)) => value.unwrap_or(Value::ZERO),
            (true, Err(Flow::Return(value))) => value,
            _ => Value::ZERO,
        }
    }
}

impl Machine<'_> {
    fn eval(&mut self, node: u32) -> Result<Value, Flow> {
        Ok(self.step(node)?.unwrap_or(Value::ZERO))
    }

    fn num(&mut self, node: u32) -> Result<f32, Flow> {
        Ok(self.eval(node)?.num())
    }

    fn step(&mut self, node: u32) -> Step {
        let program = self.program;
        Ok(Some(match program.nodes[node as usize] {
            Node::Const(value) => value,
            Node::This => Value::Num(self.this),
            Node::Var { slot, path } => return Ok(self.variable(slot, program.list(path))),
            Node::Temp { slot, path } => return Ok(self.temps.get(slot, program.list(path))),
            Node::Context(id) => return Ok(self.host.context(Context(id))),
            Node::Query { id, args } => {
                let base = self.stack.len();
                for &arg in program.list(args) {
                    let value = self.eval(arg)?;
                    self.stack.push(value);
                }
                let value = self.host.query(Query(id), &self.stack[base..], &mut Structs(self.temps));
                self.stack.truncate(base);
                value
            }
            Node::Math { function, args, count } => {
                let mut values = [0.0; 3];
                for (value, &arg) in values.iter_mut().zip(&args[..usize::from(count)]) {
                    *value = self.num(arg)?;
                }
                if matches!(function, MathFn::DieRoll | MathFn::DieRollInteger) {
                    values[0] = self.spend(values[0]) as f32;
                }
                let host = self.host;
                Value::Num(function.call(values, &|| host.random()))
            }
            Node::Not(operand) => Value::flag(!self.eval(operand)?.truthy()),
            Node::Neg(operand) => Value::Num(-self.num(operand)?),
            Node::Binary(op, left, right) => self.binary(op, left, right)?,
            Node::Coalesce(left, right) => match self.step(left)? {
                Some(value) => value,
                None => return self.step(right),
            },
            Node::Ternary(condition, then, otherwise) => {
                return if self.eval(condition)?.truthy() { self.step(then) } else { self.step(otherwise) };
            }
            Node::When(condition, then) => {
                if self.eval(condition)?.truthy() {
                    return self.step(then);
                }
                Value::ZERO
            }
            Node::Assign { target, value } => {
                let value = self.eval(value)?;
                self.assign(target, value)
            }
            Node::Block(statements) => {
                for &statement in program.list(statements) {
                    self.step(statement)?;
                }
                Value::ZERO
            }
            Node::Loop { count, body } => {
                // A fraction of a pass is a pass.
                let count = self.num(count)?.ceil();
                for _ in 0..self.spend(count) {
                    match self.step(body) {
                        Err(Flow::Break) => break,
                        Ok(_) | Err(Flow::Continue) => {}
                        Err(left) => return Err(left),
                    }
                }
                Value::ZERO
            }
            Node::ForEach { variable, array, body } => {
                let array = self.eval(array)?;
                let mut index = 0;
                // Read by position each pass: the body may assign over the variable holding the array.
                while let Some(item) = self.item(array, index).filter(|_| self.spend(1.0) == 1) {
                    self.assign(variable, item);
                    match self.step(body) {
                        Err(Flow::Break) => break,
                        Ok(_) | Err(Flow::Continue) => {}
                        Err(left) => return Err(left),
                    }
                    index += 1;
                }
                Value::ZERO
            }
            Node::Break => return Err(Flow::Break),
            Node::Continue => return Err(Flow::Continue),
            Node::Return(value) => return Err(Flow::Return(self.eval(value)?)),
            Node::Index { items, index } => {
                let items = program.list(items);
                // Indices wrap; negative ones read the first element.
                let index = self.num(index)?.max(0.0) as usize % items.len();
                return self.step(items[index]);
            }
            Node::Arrow { entity, of } => return self.arrow(entity, of),
        }))
    }

    fn item(&self, array: Value, index: usize) -> Option<Value> {
        let Value::Array(list) = array else { return None };
        let store = match &self.vars {
            _ if list.scratch => &*self.temps,
            Vars::Own(store) => &**store,
            Vars::Other(store) => *store,
        };
        store.members(list).get(index).map(|(_, item)| *item)
    }

    /// Takes up to `wanted` repeats out of what the evaluation has left.
    fn spend(&mut self, wanted: f32) -> u32 {
        let granted = (wanted.max(0.0) as u32).min(self.repeats);
        self.repeats -= granted;
        granted
    }

    fn binary(&mut self, op: Op, left: u32, right: u32) -> Result<Value, Flow> {
        let a = self.eval(left)?;
        match op {
            Op::Or if a.truthy() => return Ok(Value::flag(true)),
            Op::And if !a.truthy() => return Ok(Value::flag(false)),
            _ => {}
        }
        let b = self.eval(right)?;
        // A struct is neither equal nor unequal to anything, itself included.
        let comparable = a.storage().is_none() && b.storage().is_none();
        let (x, y) = (a.num(), b.num());
        Ok(match op {
            Op::Or | Op::And => Value::flag(b.truthy()),
            Op::Eq => Value::flag(comparable && a == b),
            Op::Ne => Value::flag(comparable && a != b),
            Op::Lt => Value::flag(x < y),
            Op::Le => Value::flag(x <= y),
            Op::Gt => Value::flag(x > y),
            Op::Ge => Value::flag(x >= y),
            Op::Add => Value::Num(x + y),
            Op::Sub => Value::Num(x - y),
            Op::Mul => Value::Num(x * y),
            Op::Div => Value::Num(if y == 0.0 { 0.0 } else { x / y }),
            Op::DivByMagnitude => Value::Num(if y == 0.0 { 0.0 } else { x / y.abs() }),
        })
    }

    fn variable(&mut self, slot: u32, path: &[u32]) -> Option<Value> {
        match &self.vars {
            Vars::Own(store) => store.get(slot, path),
            // Its structs live in a store this evaluation cannot name, so they are copied out.
            Vars::Other(store) => store.get(slot, path).map(|value| self.temps.import(value, Some(store))),
        }
    }

    /// The value of an assignment is what was stored.
    fn assign(&mut self, target: u32, value: Value) -> Value {
        // BDS stores 0 rather than a not-a-number or an infinity.
        let value = match value {
            Value::Num(n) if !n.is_finite() => Value::ZERO,
            other => other,
        };
        let program = self.program;
        match program.nodes[target as usize] {
            Node::Temp { slot, path } => {
                let value = if self.temps.owns(value) { self.temps.import(value, None) } else { self.import_temp(value) };
                self.temps.set(slot, program.list(path), value);
            }
            Node::Var { slot, path } => {
                if let Vars::Own(store) = &mut self.vars {
                    let value = if store.owns(value) { store.import(value, None) } else { store.import(value, Some(self.temps)) };
                    store.set(slot, program.list(path), value);
                }
            }
            _ => unreachable!("the parser only assigns to variables"),
        }
        value
    }

    fn import_temp(&mut self, value: Value) -> Value {
        match &self.vars {
            Vars::Own(store) => self.temps.import(value, Some(store)),
            Vars::Other(store) => self.temps.import(value, Some(store)),
        }
    }

    fn arrow(&mut self, entity: u32, of: u32) -> Step {
        let Value::Entity(entity) = self.eval(entity)? else { return Ok(None) };
        let Some((host, variables)) = self.host.entity(entity) else { return Ok(None) };
        let mut other = Machine { program: self.program, host, vars: Vars::Other(&variables.0), temps: self.temps, stack: self.stack, this: self.this, repeats: self.repeats };
        let value = other.step(of);
        self.repeats = other.repeats;
        value
    }
}
