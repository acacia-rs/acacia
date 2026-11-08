//! Compares the port tick by tick with traces recorded from the 26.3 client (bridge tools/move-trace.sh).
//! `ACACIA_TRACE=<file>` with `ACACIA_COURSE=<file>` replays one uncompressed recording instead.

mod trace;

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use flate2::read::GzDecoder;

fn gunzip(path: &Path) -> String {
    let mut text = String::new();
    GzDecoder::new(File::open(path).expect("open fixture")).read_to_string(&mut text).expect("read fixture");
    text
}

#[test]
fn recorded_traces_match_bit_for_bit() {
    if let Ok(path) = std::env::var("ACACIA_TRACE") {
        let course = std::env::var("ACACIA_COURSE").map_or(String::new(), |c| fs::read_to_string(c).expect("read course"));
        let wrong = trace::replay(&fs::read_to_string(path).expect("read trace"), &course);
        assert_eq!(wrong, 0);
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/traces");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("list traces")
        .filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".trace.gz").map(str::to_owned))
        .collect();
    names.sort();
    assert!(!names.is_empty());
    let mut failed = Vec::new();
    for name in names {
        println!("== {name}");
        let trace_text = gunzip(&dir.join(format!("{name}.trace.gz")));
        let course = gunzip(&dir.join(format!("{name}.course.gz")));
        if trace::replay(&trace_text, &course) != 0 {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "traces with differences: {failed:?}");
}
