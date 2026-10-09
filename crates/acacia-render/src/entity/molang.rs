//! How the entity code uses `acacia-molang`: compiling the pack's expressions while it loads, and
//! evaluating them against an entity whose state the caller answers for by query name.

use std::cell::RefCell;

pub use acacia_molang::Program;
use acacia_molang::{Arrays, Compiler, Env, Host, Query, Scratch, Structs, Symbol, Variable, Variables};

/// What a query answers.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(f32),
    Text(String),
}

impl Value {
    pub fn truthy(&self) -> bool {
        match self {
            Value::Num(n) => *n != 0.0,
            Value::Text(t) => !t.is_empty(),
        }
    }

    pub fn num(&self) -> f32 {
        match self {
            Value::Num(n) => *n,
            Value::Text(_) => 0.0,
        }
    }
}

/// The compiler while a pack loads; shared, since the loaders compile from inside iterator chains.
pub(super) type Loading = RefCell<Compiler>;

/// `None` for a source that does not compile: the caller then leaves the bone alone, the animation
/// off or the layer out.
pub(super) fn compile(loading: &Loading, source: &str, arrays: Option<&Arrays>) -> Option<Program> {
    let mut compiler = loading.borrow_mut();
    let compiled = match arrays {
        Some(arrays) => compiler.compile_with(source, arrays),
        None => compiler.compile(source),
    };
    compiled.inspect_err(|error| tracing::debug!(%error, source, "molang rejected")).ok()
}

struct Asked<'a> {
    compiler: &'a Compiler,
    query: &'a dyn Fn(&str) -> Value,
}

impl Host for Asked<'_> {
    fn query(&self, query: Query, args: &[acacia_molang::Value], _structs: &mut Structs<'_>) -> acacia_molang::Value {
        let name = self.compiler.query_name(query);
        // No equipment is tracked: nothing is held or worn, and nothing is named.
        if matches!(name, "get_equipped_item_name" | "get_name") {
            return acacia_molang::Value::Str(Symbol::EMPTY);
        }
        // Asked once per kind listed.
        if name == "is_riding_any_entity_of_type" {
            let rides = |kind: &acacia_molang::Value| match kind {
                acacia_molang::Value::Str(kind) => (self.query)(&format!("{name}:{}", self.compiler.text(*kind))).truthy(),
                _ => false,
            };
            return acacia_molang::Value::Num(f32::from(u8::from(args.iter().any(rides))));
        }
        let answer = match args.first() {
            Some(acacia_molang::Value::Str(argument)) => (self.query)(&format!("{name}:{}", self.compiler.text(*argument))),
            _ => (self.query)(name),
        };
        match answer {
            Value::Num(n) => acacia_molang::Value::Num(n),
            Value::Text(text) => acacia_molang::Value::Str(self.compiler.find(&text).unwrap_or(Symbol::OTHER)),
        }
    }
}

/// One entity's evaluations for a frame: its variables live as long as this does.
pub struct Scope<'a> {
    compiler: &'a Compiler,
    /// Entity state by query name without the `query.` prefix; a string argument is appended
    /// after a colon (`property:minecraft:has_nectar`). Unknown names should answer 0.
    pub query: &'a dyn Fn(&str) -> Value,
    variables: Variables,
    scratch: Scratch,
    /// What `this` reads: the current value of the animation channel being evaluated.
    pub this: f32,
}

impl<'a> Scope<'a> {
    pub(super) fn new(compiler: &'a Compiler, query: &'a dyn Fn(&str) -> Value) -> Scope<'a> {
        Scope { compiler, query, variables: Variables::new(), scratch: Scratch::new(), this: 0.0 }
    }

    pub(super) fn set(&mut self, variable: Variable, value: f32) {
        self.variables.set(variable, value);
    }

    fn eval(&mut self, program: &Program) -> acacia_molang::Value {
        let host = Asked { compiler: self.compiler, query: self.query };
        program.eval(&mut Env { host: &host, variables: &mut self.variables, scratch: &mut self.scratch, this: self.this })
    }

    /// For scripts, which are run for the variables they set.
    pub(super) fn run(&mut self, program: &Program) {
        self.eval(program);
    }

    pub(super) fn num(&mut self, program: &Program) -> f32 {
        self.eval(program).num()
    }

    pub(super) fn truthy(&mut self, program: &Program) -> bool {
        self.eval(program).truthy()
    }

    /// The resource a render controller picked, such as `texture.default`.
    pub(super) fn resource(&mut self, program: &Program) -> Option<&'a str> {
        match self.eval(program) {
            acacia_molang::Value::Str(name) => Some(self.compiler.text(name)),
            _ => None,
        }
    }
}
