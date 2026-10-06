//! Generated programs through this crate and molangx; a disagreement is a crash.
#![no_main]

use acacia_molang_fuzz::{Source, differ};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|source: Source| {
    if let Err(disagreement) = differ::compare(&source.0) {
        panic!("{disagreement}");
    }
});
