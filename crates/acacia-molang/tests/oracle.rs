//! Every case BDS was asked (`tests/oracle/*.bds`, written by `tools/molang-oracle`) must come out
//! the same here, under the engine version the file was asked at.

use std::path::Path;

use acacia_molang::{Compiler, Engine, Env, Error, NoHost, Scratch, Value, Variables};

/// Cases this crate knowingly answers differently, with the reason.
const SKIPPED: [(&str, &str); 3] = [
    ("t.x = 4; => t.x ?? -1", "a temp read by a later expression: BDS answers 1, which nothing explains"),
    ("v.b = q.is_alive ?? 4; => v.b", "needs a host that answers `query.is_alive`"),
    ("this", "BDS has no `this` where the oracle evaluates"),
];

/// Malformed sources that engines before 1.17.40 evaluated to something; here they are errors at every version.
const ONCE_TOLERATED: [&str; 2] = ["1 + (2 3)", "'a' < 'b'"];

struct Session {
    compiler: Compiler,
    variables: Variables,
    scratch: Scratch,
}

impl Session {
    fn run(&mut self, source: &str) -> Result<Value, Error> {
        let program = self.compiler.compile(source)?;
        Ok(program.eval(&mut Env { host: &NoHost, variables: &mut self.variables, scratch: &mut self.scratch, this: 0.0 }))
    }
}

/// A case in the oracle's syntax (`tools/molang-oracle/README.md`).
fn evaluate(case: &str, engine: Engine) -> Result<Value, Error> {
    let mut session = Session { compiler: Compiler::new(), variables: Variables::new(), scratch: Scratch::new() };
    session.compiler.engine = engine;
    if let Some(complex) = case.strip_prefix("truthy: ") {
        return session.run(complex).map(|value| Value::from(value.truthy()));
    }
    match case.split_once(" => ") {
        Some((setup, read)) => {
            session.run(setup)?;
            session.run(read)
        }
        None => session.run(case),
    }
}

/// Within a few representable steps: BDS's math library does not round like Rust's.
fn close(got: f32, expected: f32) -> bool {
    let steps = |n: f32| if n < 0.0 { -i64::from(n.to_bits() & 0x7fff_ffff) } else { i64::from(n.to_bits()) };
    got == expected || (steps(got) - steps(expected)).abs() <= 4
}

fn mismatch(row: &str, engine: Engine) -> Option<String> {
    let mut columns = row.split('\t');
    let (expected, case) = (columns.next()?, columns.next()?);
    let bds_complained = columns.next().is_some();
    if SKIPPED.iter().any(|(skipped, _)| *skipped == case) || (engine < Engine(1, 17, 40) && ONCE_TOLERATED.contains(&case)) {
        return None;
    }
    let got = evaluate(case, engine);
    let fine = match (expected, &got) {
        ("error", got) => got.is_err(),
        ("unset", got) => !matches!(got, Ok(Value::Num(_))),
        // The float property BDS reports through clamps infinities and not-a-number to its range.
        ("1e+30" | "-1e+30", Ok(_)) => true,
        (number, Ok(value)) => matches!(value, Value::Num(got) if close(*got, number.parse().expect(row))),
        // BDS logged an error and went on with a value; rejecting the source instead is fine.
        (_, Err(_)) => bds_complained,
    };
    (!fine).then(|| format!("{case}\n    BDS {expected}, here {got:?}"))
}

/// `# BDS 1.26.52.3, engine 1.13.0; ...`
fn engine_of(header: &str) -> Engine {
    let version = header.split_once("engine ").and_then(|(_, rest)| rest.split_once(';')).expect("engine in the header").0;
    let mut parts = version.split('.').map(|part| part.parse().expect("engine version"));
    Engine(parts.next().unwrap(), parts.next().unwrap(), parts.next().unwrap())
}

#[test]
fn every_case_matches_bds() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle");
    let mut files: Vec<_> = std::fs::read_dir(folder).unwrap().map(|entry| entry.unwrap().path()).filter(|path| path.extension().is_some_and(|e| e == "bds")).collect();
    files.sort();
    let (mut cases, mut wrong) = (0, Vec::new());
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let (header, rows) = text.split_once('\n').unwrap();
        let engine = engine_of(header);
        for row in rows.lines() {
            cases += 1;
            wrong.extend(mismatch(row, engine).map(|message| format!("{} {message}", file.file_name().unwrap().display())));
        }
    }
    assert!(cases > 300, "only {cases} cases found");
    assert!(wrong.is_empty(), "{} of {cases} cases differ from BDS:\n{}", wrong.len(), wrong.join("\n"));
}
