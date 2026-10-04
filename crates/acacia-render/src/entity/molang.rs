//! The slice of Molang that render controllers and the entity scripts feeding them use: numbers,
//! strings, resource names, operators, ternaries, array lookups, assignments. Names are
//! case-insensitive, so sources are lowercased before parsing. See README "Entities".

use std::collections::HashMap;

use super::molang_parse::parse_program;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(f32),
    /// A string, or a resource name such as `texture.default`.
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
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
    /// `a ?? b`: `b` when `a` is unset; unset names read as 0 here, so it tests truthiness.
    Coalesce,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Value(Value),
    This,
    /// A dotted name with call arguments: `query.is_baby`, `math.clamp(a, 0, 3)`, `texture.default`.
    Name(String, Vec<Expr>),
    /// `array.skins[index]`
    Index(String, Box<Expr>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Assign(String, Box<Expr>),
}

/// Statements; the value of the last one is the program's.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Program(pub Vec<Expr>);

impl Program {
    /// `None` for sources outside the supported slice (loops, structs, arrow access).
    pub fn parse(source: &str) -> Option<Program> {
        parse_program(&source.to_lowercase())
    }

    pub fn constant(value: f32) -> Program {
        Program(vec![Expr::Value(Value::Num(value))])
    }

    pub fn run(&self, scope: &mut Scope) -> Value {
        self.0.iter().fold(Value::Num(0.0), |_, e| eval(e, scope))
    }
}

pub type Arrays = HashMap<String, Vec<Expr>>;

pub struct Scope<'a> {
    /// Entity state by query name without the `query.` prefix; a string argument is appended
    /// after a colon (`property:minecraft:has_nectar`). Unknown names should answer 0.
    pub query: &'a dyn Fn(&str) -> Value,
    /// `variable.` and `temp.` values by the name after the prefix.
    pub variables: HashMap<String, Value>,
    pub arrays: Option<&'a Arrays>,
}

fn split(name: &str) -> (&str, &str) {
    let (prefix, rest) = name.split_once('.').unwrap_or((name, ""));
    let prefix = match prefix {
        "q" => "query",
        "v" | "t" | "temp" => "variable",
        other => other,
    };
    (prefix, rest)
}

fn eval(expr: &Expr, scope: &mut Scope) -> Value {
    let flag = |b: bool| Value::Num(f32::from(u8::from(b)));
    match expr {
        Expr::Value(v) => v.clone(),
        // Only colour channels use `this` (the current value); they are not evaluated.
        Expr::This => Value::Num(0.0),
        Expr::Not(e) => flag(!eval(e, scope).truthy()),
        Expr::Neg(e) => Value::Num(-eval(e, scope).num()),
        Expr::Ternary(c, a, b) => {
            if eval(c, scope).truthy() {
                eval(a, scope)
            } else {
                eval(b, scope)
            }
        }
        Expr::Assign(name, e) => {
            let value = eval(e, scope);
            scope.variables.insert(split(name).1.to_owned(), value.clone());
            value
        }
        Expr::Index(name, index) => {
            let index = eval(index, scope).num().max(0.0) as usize;
            let item = scope.arrays.and_then(|a| a.get(name)).filter(|a| !a.is_empty()).map(|a| a[index % a.len()].clone());
            item.map_or(Value::Num(0.0), |e| eval(&e, scope))
        }
        Expr::Binary(op, a, b) => {
            let a = eval(a, scope);
            match op {
                Op::Or if a.truthy() => return flag(true),
                Op::And if !a.truthy() => return flag(false),
                Op::Coalesce if a.truthy() => return a,
                _ => {}
            }
            let b = eval(b, scope);
            let (x, y) = (a.num(), b.num());
            match op {
                Op::Or | Op::And => flag(b.truthy()),
                Op::Coalesce => b,
                Op::Eq => flag(a == b),
                Op::Ne => flag(a != b),
                Op::Lt => flag(x < y),
                Op::Le => flag(x <= y),
                Op::Gt => flag(x > y),
                Op::Ge => flag(x >= y),
                Op::Add => Value::Num(x + y),
                Op::Sub => Value::Num(x - y),
                Op::Mul => Value::Num(x * y),
                Op::Div => Value::Num(if y == 0.0 { 0.0 } else { x / y }),
            }
        }
        Expr::Name(name, args) => {
            let args: Vec<Value> = args.iter().map(|a| eval(a, scope)).collect();
            match split(name) {
                // No equipment is tracked: nothing is held or worn, and nothing is named.
                ("query", "get_equipped_item_name" | "get_name") => Value::Text(String::new()),
                ("query", rest) => match args.first() {
                    Some(Value::Text(arg)) => (scope.query)(&format!("{rest}:{arg}")),
                    _ => (scope.query)(rest),
                },
                ("variable", rest) => scope.variables.get(rest).cloned().unwrap_or(Value::Num(0.0)),
                ("math", rest) => Value::Num(math(rest, &args)),
                ("geometry" | "texture" | "material", _) => Value::Text(name.clone()),
                _ => Value::Num(0.0),
            }
        }
    }
}

/// Angles are degrees. `random` returns its lower bound: looks must not flicker between frames.
fn math(name: &str, args: &[Value]) -> f32 {
    let arg = |i: usize| args.get(i).map_or(0.0, Value::num);
    let (a, b, c) = (arg(0), arg(1), arg(2));
    match name {
        "sin" => a.to_radians().sin(),
        "cos" => a.to_radians().cos(),
        "floor" => a.floor(),
        "ceil" => a.ceil(),
        "round" => a.round(),
        "trunc" => a.trunc(),
        "abs" => a.abs(),
        "sqrt" => a.max(0.0).sqrt(),
        "min" => a.min(b),
        "max" => a.max(b),
        "pow" => a.powf(b),
        "mod" if b != 0.0 => a % b,
        "clamp" => a.max(b).min(c),
        "lerp" => a + (b - a) * c,
        "pi" => std::f32::consts::PI,
        "random" | "random_integer" => a,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str, query: &dyn Fn(&str) -> Value, arrays: &Arrays) -> Value {
        let mut scope = Scope { query, variables: HashMap::new(), arrays: Some(arrays) };
        Program::parse(source).unwrap_or_else(|| panic!("parse {source}")).run(&mut scope)
    }

    fn text(s: &str) -> Value {
        Value::Text(s.to_owned())
    }

    #[test]
    fn render_controller_expressions_pick_resources() {
        let names = |list: &[&str]| list.iter().map(|n| Expr::Name((*n).to_owned(), vec![])).collect::<Vec<_>>();
        let arrays: Arrays = [
            ("array.skins".to_owned(), names(&["texture.white", "texture.black", "texture.red"])),
            ("array.tame".to_owned(), names(&["texture.white_tame", "texture.black_tame"])),
        ]
        .into();
        let cat = |name: &str| match name {
            "variant" => Value::Num(4.0),
            "is_tamed" => Value::Num(1.0),
            "property:minecraft:climate_variant" => text("warm"),
            _ => Value::Num(0.0),
        };
        assert_eq!(run("t.v = q.property('minecraft:climate_variant'); t.v == 'warm' ? 1 : 2", &cat, &Arrays::new()), Value::Num(1.0));
        let nested = "query.is_tamed ? query.is_baby ? Array.baby_tame[query.variant] : Array.tame[query.variant] : Array.skins[query.variant]";
        // Indices wrap: variant 4 of two.
        assert_eq!(run(nested, &cat, &arrays), text("texture.white_tame"));
        assert_eq!(run("Array.skins[query.variant]", &cat, &arrays), text("texture.black"));
        assert_eq!(run("query.is_baby ? Geometry.baby : Geometry.default", &cat, &arrays), text("geometry.default"));
        assert_eq!(run("Array.skins[query.property('minecraft:has_nectar') + q.is_tamed*2]", &cat, &arrays), text("texture.red"));
        assert_eq!(run("Array.skins[query.variant]", &cat, &Arrays::new()), Value::Num(0.0));
    }

    #[test]
    fn scripts_assign_variables_and_operators_follow_precedence() {
        let villager = |name: &str| Value::Num(if name == "variant" { 5.0 } else { 0.0 });
        let script = "variable.num_professions = 15; variable.profession_index = (query.variant < variable.num_professions ? query.variant : 0);";
        let visible = "!query.is_baby && v.profession_index != 0 && variable.profession_index != 14";
        let mut scope = Scope { query: &villager, variables: HashMap::new(), arrays: None };
        Program::parse(script).unwrap().run(&mut scope);
        assert_eq!(scope.variables["profession_index"], Value::Num(5.0));
        assert_eq!(Program::parse(visible).unwrap().run(&mut scope), Value::Num(1.0));

        let none = |_: &str| Value::Num(0.0);
        let empty = Arrays::new();
        assert_eq!(run("1 + 2 * 3 - -4 / 2", &none, &empty), Value::Num(9.0));
        assert_eq!(run("query.is_baby ? 2.0 : 0.9375", &none, &empty), Value::Num(0.9375));
        assert_eq!(run("1.0f", &none, &empty), Value::Num(1.0));
        assert_eq!(run("query.get_equipped_item_name(0, 1) == '' || query.x == 'map'", &none, &empty), Value::Num(1.0));
        assert_eq!(run("math.clamp(query.health / 25, 0, 3) >= 0 && (math.sin(90) + 1.0) * 0.5 == 1", &none, &empty), Value::Num(1.0));
        assert!(Program::parse("for_each(t.x, q.list, { v.y = 1; });").is_none());
    }
}
