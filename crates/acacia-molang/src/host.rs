use crate::compiler::{Context, Query, Variable};
use crate::store::Store;
use crate::value::{Symbol, Value};

/// What the game answers during an evaluation. Every method has the answer of a host that knows nothing.
pub trait Host {
    /// A struct result is built with `structs`.
    fn query(&self, _query: Query, _args: &[Value], _structs: &mut Structs<'_>) -> Value {
        Value::ZERO
    }

    /// `None` for a context variable that does not exist here (`c.x ?? 1` then yields 1).
    fn context(&self, _context: Context) -> Option<Value> {
        None
    }

    /// In [0, 1), for `math.random` and the die rolls. The default makes them answer their lower bound.
    fn random(&self) -> f32 {
        0.0
    }

    /// The entity behind a [`Value::Entity`], for `entity->expression`. Which of its variables
    /// others may read (the "public" ones) is the host's to decide.
    fn entity(&self, _entity: u32) -> Option<(&dyn Host, &Variables)> {
        None
    }

    /// `entity->variable.path = value`. The value is never a struct or an array.
    fn assign(&self, _entity: u32, _variable: Variable, _path: &[Symbol], _value: Value) {}
}

/// A host with no queries, context variables or entities.
pub struct NoHost;

impl Host for NoHost {}

/// An entity's `variable.` values. They belong to the [`crate::Compiler`] that numbered them.
#[derive(Debug, Default)]
pub struct Variables(pub(crate) Store);

impl Variables {
    pub fn new() -> Variables {
        Variables::default()
    }

    pub fn get(&self, variable: Variable) -> Option<Value> {
        self.0.get(variable.0, &[])
    }

    /// A struct value has to come from this `Variables`; build one with [`Variables::set_member`].
    pub fn set(&mut self, variable: Variable, value: impl Into<Value>) {
        self.set_member(variable, &[], value);
    }

    /// Sets `variable.a.b` for the path `[a, b]`, making the structs on the way.
    pub fn set_member(&mut self, variable: Variable, path: &[Symbol], value: impl Into<Value>) {
        let value = value.into();
        assert!(!matches!(value, Value::Struct(_)) || self.0.owns(value), "struct from another store");
        let value = self.0.import(value, None);
        self.0.set_symbols(variable.0, path, value);
    }

    pub fn member(&self, of: Value, name: Symbol) -> Option<Value> {
        self.0.owns(of).then(|| self.0.member(of, name)).flatten()
    }

    /// Forgets every variable, keeping the allocations.
    pub fn clear(&mut self) {
        self.0.reset(0);
    }
}

/// Working memory for evaluations: temp variables, call arguments and the structs made on the way.
/// Reuse one across evaluations; it stops allocating once it has grown to fit.
#[derive(Debug)]
pub struct Scratch {
    pub(crate) temps: Store,
    pub(crate) stack: Vec<Value>,
}

impl Default for Scratch {
    fn default() -> Scratch {
        Scratch { temps: Store::scratch(), stack: Vec::new() }
    }
}

impl Scratch {
    pub fn new() -> Scratch {
        Scratch::default()
    }

    /// A member of a struct the last evaluation returned.
    pub fn member(&self, of: Value, name: Symbol) -> Option<Value> {
        self.temps.owns(of).then(|| self.temps.member(of, name)).flatten()
    }
}

/// Builds the struct a query returns.
pub struct Structs<'a>(pub(crate) &'a mut Store);

impl Structs<'_> {
    /// Members may be structs made by this builder.
    pub fn make(&mut self, members: &[(Symbol, Value)]) -> Value {
        let made = self.0.new_struct();
        for &(name, value) in members {
            self.0.set_member(made, name, value);
        }
        Value::Struct(made)
    }

    /// An array of entities, for `for_each` to walk.
    pub fn entities(&mut self, entities: &[u32]) -> Value {
        let made = self.0.new_struct();
        for &entity in entities {
            self.0.push_member(made, Value::Entity(entity));
        }
        Value::Array(made)
    }
}

/// Everything one evaluation reads and writes.
pub struct Env<'a> {
    pub host: &'a dyn Host,
    pub variables: &'a mut Variables,
    pub scratch: &'a mut Scratch,
    /// What `this` reads: the current value of the animation channel being evaluated.
    pub this: f32,
}
