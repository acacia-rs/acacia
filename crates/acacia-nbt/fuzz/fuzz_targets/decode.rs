//! Arbitrary bytes through both flavours' `read` and `skip`.
#![no_main]

use acacia_nbt::{LittleEndian, Network};
use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../tests/common/props.rs"]
mod props;

fuzz_target!(|data: &[u8]| {
    props::decode::<Network>(data);
    props::decode::<LittleEndian>(data);
});
