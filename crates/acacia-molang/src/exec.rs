//! Statements, variables and `->`: the parts of evaluation that change something. See `eval.rs`.

use crate::compiler::Variable;
use crate::eval::{Flow, Machine, Vars};
use crate::program::Node;
use crate::store::Store;
use crate::value::{StructRef, Symbol, Value};

/// BDS stores 0 rather than a not-a-number or an infinity.
fn storable(value: Value) -> Value {
    match value {
        Value::Num(n) if !n.is_finite() => Value::ZERO,
        other => other,
    }
}

impl Machine<'_> {
    /// A statement. The value is that of an assignment or an expression, else 0.
    #[inline(never)]
    pub(crate) fn run(&mut self, node: u32) -> Value {
        let program = self.program;
        match program.nodes[node as usize] {
            Node::Assign { target, value } => {
                let value = self.value(value);
                // A `return` inside the right side leaves before anything is stored.
                if self.flow != Flow::Running {
                    return Value::ZERO;
                }
                match program.nodes[target as usize] {
                    Node::Arrow { entity, of } => {
                        let entity = self.value(entity);
                        self.assign_other(entity, of, value)
                    }
                    _ => self.assign(target, value),
                }
            }
            Node::Block(statements) => {
                for &statement in program.list(statements) {
                    self.run(statement);
                    if self.flow != Flow::Running {
                        break;
                    }
                }
                Value::ZERO
            }
            Node::Loop { count, body } => {
                // A fraction of a pass is a pass; BDS counts a not-a-number down forever.
                let count = match self.num(count) {
                    n if n.is_nan() => f32::INFINITY,
                    n => n.ceil(),
                };
                for _ in 0..self.spend(count) {
                    self.run(body);
                    if self.leaves_loop() {
                        break;
                    }
                }
                Value::ZERO
            }
            Node::ForEach { variable, array, body } => {
                let array = self.value(array);
                let mut index = 0;
                // Read by position each pass: the body may assign over the variable holding the array.
                while let Some(item) = self.item(array, index).filter(|_| self.spend(1.0) == 1) {
                    self.assign(variable, item);
                    self.value(body);
                    if self.leaves_loop() {
                        break;
                    }
                    index += 1;
                }
                Value::ZERO
            }
            Node::Return(value) => {
                let value = self.value(value);
                if self.flow == Flow::Running {
                    (self.flow, self.returned) = (Flow::Return, value);
                }
                Value::ZERO
            }
            // Only while running: the condition in front of one may have ended the program.
            Node::Break | Node::Continue if self.flow != Flow::Running => Value::ZERO,
            Node::Break => {
                self.flow = Flow::Break;
                Value::ZERO
            }
            Node::Continue => {
                self.flow = Flow::Continue;
                Value::ZERO
            }
            // An expression used as a statement.
            _ => self.value(node),
        }
    }

    /// After a loop body: takes a `break` or `continue` off, and says whether the loop ends.
    fn leaves_loop(&mut self) -> bool {
        let flow = std::mem::replace(&mut self.flow, Flow::Running);
        if matches!(flow, Flow::Return | Flow::Abort) {
            self.flow = flow;
        }
        flow != Flow::Continue && flow != Flow::Running
    }

    /// Takes up to `wanted` repeats out of what the evaluation has left.
    pub(crate) fn spend(&mut self, wanted: f32) -> u32 {
        let granted = (wanted.max(0.0) as u32).min(self.repeats);
        self.repeats -= granted;
        granted
    }

    fn item(&self, array: Value, index: usize) -> Option<Value> {
        let Value::Array(list) = array else { return None };
        self.store(list).members(list).get(index).map(|(_, item)| *item)
    }

    /// The store a struct or an array lives in.
    pub(crate) fn store(&self, held: StructRef) -> &Store {
        match &self.vars {
            _ if held.scratch() => self.temps,
            Vars::Own(store) => store,
            Vars::Other(store) => store,
        }
    }

    pub(crate) fn variable(&mut self, slot: u32, path: &[u32]) -> Option<Value> {
        match &self.vars {
            Vars::Own(store) => store.get(slot, path),
            // Its structs live in a store this evaluation cannot name, so they are copied out.
            Vars::Other(store) => store.get(slot, path).map(|value| self.temps.import(value, Some(store))),
        }
    }

    /// The value of an assignment is what was stored.
    fn assign(&mut self, target: u32, value: Value) -> Value {
        let value = storable(value);
        let program = self.program;
        let held = value.storage().is_some();
        match program.nodes[target as usize] {
            Node::Temp { slot, path } if path.len == 0 && self.temps.set_plain(slot, value) => {}
            Node::Temp { slot, path } => {
                let value = match &self.vars {
                    _ if !held => value,
                    _ if self.temps.owns(value) => self.temps.import(value, None),
                    Vars::Own(store) => self.temps.import(value, Some(store)),
                    Vars::Other(store) => self.temps.import(value, Some(store)),
                };
                self.temps.set(slot, program.list(path), value);
            }
            Node::Var { slot, path } => {
                if let Vars::Own(store) = &mut self.vars {
                    if path.len == 0 && store.set_plain(slot, value) {
                        return value;
                    }
                    let value = match held {
                        false => value,
                        true if store.owns(value) => store.import(value, None),
                        true => store.import(value, Some(self.temps)),
                    };
                    store.set(slot, program.list(path), value);
                }
            }
            _ => unreachable!("the parser only assigns to variables"),
        }
        value
    }

    /// `entity->variable.path = value`, handed to the host. Structs and arrays are not handed over:
    /// they live in this evaluation's stores.
    fn assign_other(&self, entity: Value, target: u32, value: Value) -> Value {
        let value = storable(value);
        let (Value::Entity(entity), Node::Var { slot, path }) = (entity, self.program.nodes[target as usize]) else { return value };
        let path = self.program.list(path);
        let mut members = [Symbol::EMPTY; 8];
        if value.storage().is_none() && path.len() <= members.len() {
            for (member, &id) in members.iter_mut().zip(path) {
                *member = Symbol(id);
            }
            self.host.assign(entity, Variable(slot), &members[..path.len()], value);
        }
        value
    }

    pub(crate) fn arrow(&mut self, entity: u32, of: u32) -> Option<Value> {
        let Value::Entity(entity) = self.value(entity) else { return None };
        let (host, variables) = self.host.entity(entity)?;
        let mut other = Machine {
            program: self.program,
            host,
            vars: Vars::Other(&variables.0),
            temps: self.temps,
            stack: self.stack,
            this: self.this,
            repeats: self.repeats,
            flow: Flow::Running,
            returned: Value::ZERO,
        };
        let value = other.maybe(of);
        self.repeats = other.repeats;
        value
    }
}
