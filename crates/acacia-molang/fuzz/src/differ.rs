//! One source through this crate and through molangx, each from the same fresh state. molangx is a
//! second opinion, not the truth: a disagreement goes to BDS (`tools/molang-oracle`), and the loser
//! gets fixed here or listed in the README.

use acacia_molang::{Compiler, Env, NoHost, Scratch, Value, Variables};
use molangx::compile::{CompileOptions, compile};
use molangx::version::MolangVersion;
use molangx::vm::{NoHostEnv, Value as Theirs, VariableName};

pub const PRESET: [(&str, f32); 2] = [("a", 1.5), ("b", -2.0)];
pub const THIS: f32 = 2.5;

/// Read after the program, to see what it stored. The fallback is a number no program computes.
const PROBES: [&str; 6] = ["v.a ?? -777", "v.b ?? -777", "v.c ?? -777", "v.none ?? -777", "v.s.x ?? -777", "v.s.y ?? -777"];

struct Ours {
    compiler: Compiler,
    variables: Variables,
    scratch: Scratch,
}

impl Ours {
    /// `None` for a source that does not compile; a result that is not a number counts as 0.
    fn run(&mut self, source: &str) -> Option<f32> {
        let program = self.compiler.compile(source).ok()?;
        let env = &mut Env { host: &NoHost, variables: &mut self.variables, scratch: &mut self.scratch, this: THIS };
        Some(match program.eval(env) {
            Value::Num(number) => number,
            _ => 0.0,
        })
    }
}

fn theirs(env: &mut NoHostEnv, options: &CompileOptions, source: &str) -> Option<f32> {
    let expr = compile(source, options).expr().cloned()?;
    Some(expr.eval_f32(&mut env.cx()))
}

fn same(a: f32, b: f32) -> bool {
    a == b || (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-4 * a.abs().max(b.abs()).max(1.0)
}

/// The first thing the two disagree on: whether the source compiles, its value, or a variable it left.
pub fn compare(source: &str) -> Result<(), String> {
    let mut ours = Ours { compiler: Compiler::new(), variables: Variables::new(), scratch: Scratch::new() };
    let options = CompileOptions::server(MolangVersion::LATEST);
    let mut env = NoHostEnv { this: THIS, ..NoHostEnv::new() };
    for (name, value) in PRESET {
        let variable = ours.compiler.variable(name);
        ours.variables.set(variable, value);
        env.variables.set(VariableName::new(name), Theirs::Float(value));
    }
    for (index, step) in std::iter::once(source).chain(PROBES).enumerate() {
        match (ours.run(step), theirs(&mut env, &options, step)) {
            (Some(a), Some(b)) if same(a, b) => {}
            // molangx stores a not-a-number as it is; BDS stores 0 (`tests/oracle/math.bds`).
            (Some(0.0), Some(b)) if index > 0 && b.is_nan() => {}
            (None, None) => return Ok(()),
            (a, b) => return Err(format!("`{step}` after `{source}`: here {a:?}, molangx {b:?} (None: rejected)")),
        }
    }
    Ok(())
}
