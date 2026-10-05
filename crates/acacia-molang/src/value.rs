/// An interned string; [`crate::Compiler::symbol`] makes one and [`crate::Compiler::text`] reads it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol(pub(crate) u32);

impl Symbol {
    pub const EMPTY: Symbol = Symbol(0);
}

/// A struct in [`crate::Variables`] or, when it was just produced by an evaluation, in its [`crate::Scratch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructRef(u32);

impl StructRef {
    const SCRATCH: u32 = 1 << 31;

    pub(crate) fn new(index: u32, scratch: bool) -> StructRef {
        StructRef(index | if scratch { StructRef::SCRATCH } else { 0 })
    }

    pub(crate) fn index(self) -> usize {
        (self.0 & !StructRef::SCRATCH) as usize
    }

    pub(crate) fn scratch(self) -> bool {
        self.0 & StructRef::SCRATCH != 0
    }
}

// The evaluator returns these from every node; past 8 bytes Windows hands them back through memory,
// which made evaluation several times slower.
const _: () = assert!(size_of::<Option<Value>>() == 8);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Num(f32),
    /// A string, or a resource name such as `texture.default` (lowercased, prefix included).
    Str(Symbol),
    /// Members are read with [`crate::Variables::member`] or [`crate::Scratch::member`].
    Struct(StructRef),
    /// Entities, as a query such as `get_nearby_entities` returns them, for `for_each`.
    Array(StructRef),
    /// An entity the host handed out, for the left side of `->`.
    Entity(u32),
}

/// `Value::Struct` or `Value::Array`.
pub(crate) type Wrap = fn(StructRef) -> Value;

impl Value {
    pub const ZERO: Value = Value::Num(0.0);

    pub fn num(self) -> f32 {
        match self {
            Value::Num(n) => n,
            _ => 0.0,
        }
    }

    pub fn truthy(self) -> bool {
        match self {
            Value::Num(n) => n != 0.0,
            Value::Str(s) => s != Symbol::EMPTY,
            Value::Struct(_) | Value::Array(_) => false,
            Value::Entity(_) => true,
        }
    }

    /// Where a struct or an array keeps its contents, and which of the two it is.
    pub(crate) fn storage(self) -> Option<(StructRef, Wrap)> {
        match self {
            Value::Struct(r) => Some((r, Value::Struct)),
            Value::Array(r) => Some((r, Value::Array)),
            _ => None,
        }
    }

    pub(crate) fn flag(set: bool) -> Value {
        Value::Num(f32::from(u8::from(set)))
    }
}

impl From<f32> for Value {
    fn from(n: f32) -> Value {
        Value::Num(n)
    }
}

impl From<bool> for Value {
    fn from(set: bool) -> Value {
        Value::flag(set)
    }
}
