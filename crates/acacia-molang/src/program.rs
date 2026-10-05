use crate::math::MathFn;
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Op {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    /// Packs older than 1.19.60 divide by the size of a computed divisor, ignoring its sign.
    DivByMagnitude,
}

/// A run of [`Program::lists`]: node indices, or the member symbols of a variable path.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct List {
    pub(crate) start: u32,
    pub(crate) len: u32,
}

/// Children are indices into [`Program::nodes`].
#[derive(Debug, Clone, Copy)]
pub(crate) enum Node {
    Const(Value),
    This,
    /// `variable.root.member.member`
    Var { slot: u32, path: List },
    Temp { slot: u32, path: List },
    Context(u32),
    Query { id: u32, args: List },
    /// `query.name.member.member`: members of the struct a query returned.
    Member { of: u32, path: List },
    Math { function: MathFn, args: [u32; 3], count: u8 },
    Not(u32),
    Neg(u32),
    Binary(Op, u32, u32),
    Coalesce(u32, u32),
    Ternary(u32, u32, u32),
    /// `a ? b`
    When(u32, u32),
    /// The target is a `Var` or `Temp` node.
    Assign { target: u32, value: u32 },
    Block(List),
    Loop { count: u32, body: u32 },
    /// `for_each(variable, array, { body })`; the variable is a `Var` or `Temp` node.
    ForEach { variable: u32, array: u32, body: u32 },
    Break,
    Continue,
    Return(u32),
    /// `array.name[index]`, the array's elements compiled in place.
    Index { items: List, index: u32 },
    Arrow { entity: u32, of: u32 },
}

/// A compiled expression; see [`crate::Compiler::compile`] and [`Program::eval`](crate::Program::eval).
#[derive(Debug, Clone)]
pub struct Program {
    pub(crate) nodes: Box<[Node]>,
    pub(crate) lists: Box<[u32]>,
    pub(crate) root: u32,
    /// Has a `;` or an assignment: its value is 0 unless a `return` runs.
    pub(crate) complex: bool,
    pub(crate) temps: u32,
}

impl Program {
    pub(crate) fn list(&self, list: List) -> &[u32] {
        &self.lists[list.start as usize..(list.start + list.len) as usize]
    }

    /// The number, when the whole program is one (`"0.5"`, or a field that was a JSON number).
    pub fn as_constant(&self) -> Option<f32> {
        match (self.complex, self.nodes[self.root as usize]) {
            (false, Node::Const(Value::Num(n))) => Some(n),
            _ => None,
        }
    }
}
