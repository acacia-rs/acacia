//! Every case BDS was asked (`tests/oracle/*.bds`, written by `tools/molang-oracle`) must come out
//! the same here, under the engine version the file was asked at.

use std::path::Path;

use acacia_molang::{Compiler, Engine, Env, Error, NoHost, Scratch, Value, Variables};

/// Cases this crate knowingly answers differently, by reason (README, "Limits").
const SKIPPED: [(&str, &[&str]); 6] = [
    ("BDS has no `this` where the oracle evaluates", &["this"]),
    ("a temp read by a later expression: BDS answers 1, which nothing explains", &["t.x = 4; => t.x ?? -1", "t.x = 1; v.x = 2; v.y = t.x; => t.x"]),
    ("a bare `break` gives BDS no number", &["break"]),
    ("needs a host that answers `query.is_alive`", &["v.b = q.is_alive ?? 4; => v.b"]),
    ("BDS takes a string's hash bits for its number", &[
        "v.e = ''; => (v.e == 0) ? 1 : 2",
        "v.e01 = ''; => (0 == v.e01) ? 1 : 2",
        "v.e04 = ''; v.h04 = -0.5; v.nz04 = math.ceil(v.h04); => (v.e04 == v.nz04) ? 1 : 2",
        "v.e27 = ''; => (v.e27 != 0) ? 1 : 2",
        "v.e29 = ''; v.z29v = 0; => (v.e29 == v.z29v) ? 1 : 2",
        "v.e29 = ''; v.z29v = 0; => (v.z29v == v.e29) ? 1 : 2",
        "v.z = 0; => (v.z == '') ? 1 : 2",
        "v.z29 = 0; => (v.z29 != '') ? 1 : 2",
        "v.one = 1; v.s = 'a'; v.f = v.s * v.one; => (v.s != v.f) ? 1 : 2",
        "v.one = 1; v.s = 'a'; v.f = v.s * v.one; => (v.s == v.f) ? 1 : 2",
        "v.one = 1; v.s28 = 'a'; v.f28 = v.s28 * v.one; v.big = math.pow(10, 35); v.g80 = v.f28 * v.big; => v.f28 == 0 ? 1 : (v.f28 < 0 ? ((v.g80 < -2 && v.g80 > -3) ? 2 : 3) : (v.f28 > 0 ? 4 : 5))",
        "v.s06 = 'a'; v.one06 = 1; v.f06 = v.s06 * v.one06; v.g06 = v.f06 * v.one06; => (v.s06 == v.g06) ? 1 : 2",
        "v.s85 = 'a'; v.zero = 0; v.g85 = v.s85 + v.zero; => v.g85 == 0 ? 1 : (v.g85 < 0 ? 2 : (v.g85 > 0 ? 3 : 4))",
    ]),
    ("a `continue` inside an operand leaves the operand where BDS keeps the loop's count", &[
        "t.st20 = 1; v.k = 0; v.i = 0; v.t20 = 7; v.a20 = 2; v.b20 = 3; loop(3, { v.i = v.i + 1; v.t20 = v.k * (v.i > 0 ? {continue;} : 0); }); t.st20 = 2; v.m20 = v.a20 * v.b20 + v.a20; => v.i == 1 ? (v.m20 == 8 ? 1 : 2) : (v.i == 3 ? (v.m20 == 8 ? 3 : 4) : 5)",
        "t.st20n = 1; v.kn = -1; v.in = 0; v.t20n = 7; loop(3, { v.in = v.in + 1; v.t20n = v.kn * (v.in > 0 ? {continue;} : 0); }); t.st20n = 2; => v.in == 1 ? 1 : (v.in == 3 ? 2 : 3)",
        "t.st20p = 1; v.kp = 0; v.ip = 0; loop(3, { v.ip = v.ip + 1; v.tp = v.kp + (v.ip > 0 ? {continue;} : 0); }); t.st20p = 2; => v.ip == 1 ? 1 : (v.ip == 3 ? 2 : 3)",
        "v.k02 = 0; v.i02 = 0; loop(3, { v.i02 = v.i02 + 1; v.t02 = v.k02 * (v.i02 > 1 ? {continue;} : 0); }); => v.i02 == 2 ? 1 : (v.i02 == 3 ? 2 : (v.i02 == 1 ? 3 : 4))",
        "v.k03 = 0; v.i03 = 0; loop(3, { v.i03 = v.i03 + 1; v.t03 = math.max(v.k03, (v.i03 > 0 ? {continue;} : 0)); }); => v.i03 == 1 ? 1 : (v.i03 == 3 ? 2 : 3)",
    ]),
];

/// Malformed sources BDS evaluated to something, some only at older engine versions; here they are
/// errors at every version.
const TOLERATED: [&str; 15] = [
    "t.st = 1; v.x = 0; v.y = 7; v.a = 0; (v.x = 1;); t.st = 2; => v.x == 1 ? (v.y == 7 ? 1 : (v.y == 0 ? 2 : (v.y == 1 ? 3 : (v.y == 2 ? 4 : 5)))) : (v.x == 0 ? (v.y == 7 ? 6 : (v.y == 0 ? 7 : (v.y == 1 ? 8 : (v.y == 2 ? 9 : 10)))) : 11)",
    "1 + (2 3)",
    "'a' < 'b'",
    "math.abs('a')",
    "math.max(1, 'a')",
    "!'a'",
    "'a' + 'b'",
    "1 + (9 10)",
    "[1 2]",
    "(1 2)",
    "temp.v = ('foo' 'bar'); return temp.v; => 0",
    "v.count = 0; loop(3, {v.count = v.count + 1; (v.count == 2) ? break + 1; }); => v.count",
    "v.count = 0; loop(3, {(v.count == 1) ? continue + 1; v.count = v.count + 1;}); => v.count",
    "t.st25 = 1; v.r25 = 9; v.r25 = math.abs(return 5); t.st25 = 2; => v.r25 == 0 ? 1 : (v.r25 == 5 ? 2 : (v.r25 == 9 ? 3 : 4))",
    "v.a = 0; v.y = 7; v.y = math.abs((v.a = 1;)); => v.a == 1 ? (v.y == 0 ? 1 : (v.y == 1 ? 2 : (v.y == 7 ? 3 : 4))) : (v.a == 0 ? 5 : 6)",
];

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
    if SKIPPED.iter().any(|(_, cases)| cases.contains(&case)) {
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
        (_, Err(_)) => bds_complained || TOLERATED.contains(&case),
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
