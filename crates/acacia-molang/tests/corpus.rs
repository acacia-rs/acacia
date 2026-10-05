//! Every Molang expression of the vanilla packs must compile. The corpus is Mojang's, so it is
//! fetched (`tools/molang-oracle/corpus.py`), not committed; without it the test passes vacuously.

use std::path::PathBuf;

use acacia_molang::{Compiler, Env, ErrorKind, NoHost, Scratch, Variables};

fn corpus() -> Option<String> {
    let default = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/molang/corpus.txt");
    std::fs::read_to_string(std::env::var_os("MOLANG_CORPUS").map_or(default, PathBuf::from)).ok()
}

#[test]
fn vanilla_expressions_compile() {
    let Some(corpus) = corpus() else {
        eprintln!("no corpus: run tools/molang-oracle/corpus.py, or set MOLANG_CORPUS");
        return;
    };
    let mut compiler = Compiler::new();
    let (mut variables, mut scratch) = (Variables::new(), Scratch::new());
    let rejected: Vec<String> = corpus
        .lines()
        .filter_map(|source| match compiler.compile(source) {
            Ok(program) => {
                // No expected value without the game behind it; this only has to come back.
                program.eval(&mut Env { host: &NoHost, variables: &mut variables, scratch: &mut scratch, this: 0.0 });
                None
            }
            // Arrays belong to the render controller a source came from, which the corpus does not keep.
            Err(acacia_molang::Error { kind: ErrorKind::UnknownArray(_), .. }) => None,
            Err(error) => Some(format!("{error}\n    {source}")),
        })
        .collect();
    assert!(rejected.is_empty(), "{} of {} expressions rejected:\n{}", rejected.len(), corpus.lines().count(), rejected.join("\n"));
}
