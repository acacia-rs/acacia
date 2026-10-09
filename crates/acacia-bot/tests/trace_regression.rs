//! Replays recorded strict-BDS sessions (`tests/traces`, see docs/testing.md "Movement physics") through the
//! current movement code: a physics change must not add server corrections we disagree with.
use std::path::Path;

use acacia_bot::trace::{self, CORRECTION_TOLERANCE};

/// (trace, mismatches it is known to keep). Known ones are the spawn tick and single open cases (see
/// docs/testing.md).
const TRACES: &[(&str, usize)] = &[
    ("fuzz-a.btrc.gz", 1),
    ("fuzz-b.btrc.gz", 0),
    ("drills-sneak-edge.btrc.gz", 1),
    ("drills-honey.btrc.gz", 1),
    ("drills-honey-top.btrc.gz", 0),
    ("drills-powder-snow.btrc.gz", 0),
    ("drills-ceiling-edge.btrc.gz", 0),
    ("drills-pinned-sprint.btrc.gz", 0),
    ("drills-flight.btrc.gz", 0),
    ("drills-dimensions.btrc.gz", 0),
];

#[test]
fn replayed_traces_keep_their_mismatch_counts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/traces");
    let mut failures = Vec::new();
    for &(name, known) in TRACES {
        let events = trace::read(&dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let report = trace::replay(&events, 1e-4, false);
        let (mismatches, _) = report.correction_mismatches(CORRECTION_TOLERANCE);
        println!("{name:28} {} of {} corrections mismatch (known {known})", mismatches.len(), report.corrections.len());
        if mismatches.len() > known {
            let ticks: Vec<_> = mismatches.iter().map(|c| c.tick).collect();
            failures.push(format!("{name}: {} mismatches, known {known}; ticks {ticks:?}", mismatches.len()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
