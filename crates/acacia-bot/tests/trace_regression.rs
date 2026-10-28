//! Replays recorded strict-BDS sessions (`tests/traces`, see docs/testing.md "Movement physics") through the
//! current movement code: a physics change must not add server corrections we disagree with.
use std::path::Path;

use acacia_bot::trace::{self, CORRECTION_TOLERANCE};

/// (trace, mismatches it is known to keep, correction tolerance). Known ones are the spawn tick and single open
/// cases (see docs/testing.md). Bit-exact (tolerance 0) traces pin ulp-level rules; most are from Windows BDS,
/// whose `sinf` differs from Linux BDS's by an ulp here and there, so they are checked at the default.
const TRACES: &[(&str, usize, f32)] = &[
    ("fuzz-a.btrc.gz", 1, CORRECTION_TOLERANCE),
    ("fuzz-b.btrc.gz", 0, CORRECTION_TOLERANCE),
    ("drills-sneak-edge.btrc.gz", 1, CORRECTION_TOLERANCE),
    ("drills-honey.btrc.gz", 1, CORRECTION_TOLERANCE),
    ("drills-honey-top.btrc.gz", 0, CORRECTION_TOLERANCE),
    ("drills-powder-snow.btrc.gz", 0, CORRECTION_TOLERANCE),
    ("drills-ceiling-edge.btrc.gz", 0, CORRECTION_TOLERANCE),
    ("drills-pinned-sprint.btrc.gz", 0, CORRECTION_TOLERANCE),
    ("drills-flight.btrc.gz", 0, CORRECTION_TOLERANCE),
    ("drills-dimensions.btrc.gz", 0, CORRECTION_TOLERANCE),
    // Human seed 1 on the course (`examples/course.rs`), Linux BDS 1.26.52.3, before BDS's sine table and the
    // correction box fit: 34 of its 35 corrections mismatched then (docs/DESIGN.md "Off-axis ulp corrections");
    // 8 until the received-yaw wrap (acacia-physics `math.rs` `wrap_degrees`).
    ("course-human-linux.btrc.gz", 6, 0.0),
];

#[test]
fn replayed_traces_keep_their_mismatch_counts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/traces");
    let mut failures = Vec::new();
    for &(name, known, tolerance) in TRACES {
        let events = trace::read(&dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let report = trace::replay(&events, 1e-4, false);
        let (mismatches, _) = report.correction_mismatches(tolerance);
        println!("{name:28} {} of {} corrections mismatch (known {known})", mismatches.len(), report.corrections.len());
        if mismatches.len() > known {
            let ticks: Vec<_> = mismatches.iter().map(|c| c.tick).collect();
            failures.push(format!("{name}: {} mismatches, known {known}; ticks {ticks:?}", mismatches.len()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
