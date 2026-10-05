//! `cargo bench -p acacia-molang`: nanoseconds per compile and per evaluation. Plain timing, no
//! harness crate. The corpus rows need `assets/molang/corpus.txt` (`tools/molang-oracle/corpus.py`).

use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use acacia_molang::{Compiler, Env, Host, Program, Query, Scratch, Structs, Value, Variables};

/// Answers every query with a number that keeps branches and divisions busy.
struct Busy;

impl Host for Busy {
    fn query(&self, query: Query, _args: &[Value], _structs: &mut Structs<'_>) -> Value {
        Value::Num(0.25 + (query.0 % 7) as f32)
    }
}

const CASES: [(&str, &str); 8] = [
    ("constant", "-20.0"),
    ("constant arithmetic", "math.sin(90) * 57.3 + 2 * 3"),
    ("one query", "query.life_time"),
    ("animation channel", "math.sin(query.life_time * 38.17) * 57.3 + this"),
    ("conditional chain", "query.is_baby ? 1.0 : query.variant == 2 ? 0.5 : query.is_tamed ? 0.25 : 0.0"),
    ("variables", "v.count = (v.count ?? 0) + 1; t.half = v.count / 2; v.half = t.half;"),
    ("struct members", "v.pos.x = q.life_time; v.pos.y = v.pos.x * 2; return v.pos.x + v.pos.y;"),
    ("loop of 16", "v.i = 0; loop(16, { v.i = v.i + math.cos(v.i); });"),
];

/// The fastest of many short timed batches, in nanoseconds per call: the minimum shrugs off a busy machine.
fn time(mut call: impl FnMut()) -> f64 {
    let mut batch = 1u32;
    loop {
        let start = Instant::now();
        (0..batch).for_each(|_| call());
        if start.elapsed() > Duration::from_millis(2) {
            break;
        }
        batch *= 2;
    }
    let best = (0..100)
        .map(|_| {
            let start = Instant::now();
            (0..batch).for_each(|_| call());
            start.elapsed()
        })
        .min()
        .unwrap();
    best.as_nanos() as f64 / f64::from(batch)
}

fn evaluate(programs: &[Program], variables: &mut Variables, scratch: &mut Scratch) {
    for program in programs {
        black_box(program.eval(&mut Env { host: &Busy, variables, scratch, this: 1.0 }));
    }
}

fn main() {
    let (mut variables, mut scratch) = (Variables::new(), Scratch::new());
    println!("{:<24}{:>12}{:>12}", "", "compile ns", "eval ns");
    for (name, source) in CASES {
        let compile = time(|| drop(black_box(Compiler::new().compile(black_box(source)))));
        let program = Compiler::new().compile(source).unwrap();
        let eval = time(|| evaluate(std::slice::from_ref(&program), &mut variables, &mut scratch));
        println!("{name:<24}{compile:>12.0}{eval:>12.1}");
    }

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/molang/corpus.txt");
    let Ok(corpus) = std::fs::read_to_string(std::env::var_os("MOLANG_CORPUS").map_or(path, PathBuf::from)) else {
        return println!("no corpus: run tools/molang-oracle/corpus.py for the vanilla rows");
    };
    let sources: Vec<&str> = corpus.lines().collect();
    let compile_all = |compiler: &mut Compiler| sources.iter().filter_map(|source| compiler.compile(source).ok()).collect::<Vec<Program>>();
    let programs = compile_all(&mut Compiler::new());
    let compile = time(|| drop(black_box(compile_all(&mut Compiler::new())))) / sources.len() as f64;
    variables.clear();
    let eval = time(|| evaluate(&programs, &mut variables, &mut scratch)) / programs.len() as f64;
    println!("{:<24}{compile:>12.0}{eval:>12.1}", format!("vanilla ({})", programs.len()));
}
