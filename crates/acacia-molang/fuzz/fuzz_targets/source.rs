//! Arbitrary text as a source, at an engine version the first byte picks: compiling and evaluating
//! it must neither panic nor run away.
#![no_main]

use acacia_molang::{Compiler, Engine, Env, Host, Query, Scratch, Structs, Value, Variables};
use libfuzzer_sys::fuzz_target;

const ENGINES: [Engine; 5] = [Engine(1, 13, 0), Engine(1, 18, 10), Engine(1, 19, 60), Engine(1, 20, 50), Engine::LATEST];

/// Answers every query, so the branches behind one are reached.
struct Busy;

impl Host for Busy {
    fn query(&self, query: Query, args: &[Value], _structs: &mut Structs<'_>) -> Value {
        Value::Num((query.0 % 5) as f32 - 1.5 + args.len() as f32)
    }

    fn random(&self) -> f32 {
        0.75
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&engine, text)) = data.split_first() else { return };
    let Ok(source) = std::str::from_utf8(text) else { return };
    let mut compiler = Compiler::new();
    compiler.engine = ENGINES[usize::from(engine) % ENGINES.len()];
    compiler.documented_queries_only = engine & 0x80 == 0;
    let Ok(program) = compiler.compile(source) else { return };
    let (mut variables, mut scratch) = (Variables::new(), Scratch::new());
    // Twice: the second run meets the variables the first one left.
    for _ in 0..2 {
        program.eval(&mut Env { host: &Busy, variables: &mut variables, scratch: &mut scratch, this: 1.0 });
    }
});
