use crate::build::{Block, Structure};

#[test]
fn round_trips_through_mcstructure_bytes() {
    let mut s = Structure::new([3, 2, 4]);
    s.set([0, 0, 0], Some(Block::new("stone", "")));
    s.set([2, 1, 3], Some(Block::new("oak_stairs", "weirdo_direction=2,upside_down_bit=1")));
    s.set([1, 0, 2], Some(Block::new("air", "")));
    let back = Structure::load(&s.to_bytes()).expect("loads");
    assert_eq!(back, s);
    assert_eq!(back.get([2, 1, 3]).and_then(|b| b.prop("upside_down_bit")), Some("1"));
    assert_eq!(back.get([1, 1, 1]), None, "structure void");
    assert_eq!(back.cells().count(), 3);
}

/// `bytes` with each `block_indices` layer an IntArray, as BDS keeps structures in its LevelDB.
fn int_array_layers(bytes: &[u8]) -> Vec<u8> {
    let tag: &[u8] = b"\x09\x0d\x00block_indices\x09";
    let at = bytes.windows(tag.len()).position(|w| w == tag).expect("block_indices");
    let mut i = at + tag.len();
    let n = i32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
    let mut out = [&bytes[..i - 1], &[0x0b], &bytes[i..i + 4]].concat();
    i += 4;
    for _ in 0..n {
        let len = i32::from_le_bytes(bytes[i + 1..i + 5].try_into().unwrap()) as usize;
        out.extend_from_slice(&bytes[i + 1..i + 5 + 4 * len]);
        i += 5 + 4 * len;
    }
    out.extend_from_slice(&bytes[i..]);
    out
}

#[test]
fn loads_layers_stored_as_int_arrays() {
    let mut s = Structure::new([2, 1, 2]);
    s.set([1, 0, 1], Some(Block::new("stone", "")));
    assert_eq!(Structure::load(&int_array_layers(&s.to_bytes())).expect("loads"), s);
}

#[test]
fn reads_cells_x_outermost_z_innermost() {
    let mut s = Structure::new([2, 2, 2]);
    s.set([1, 0, 0], Some(Block::new("stone", "")));
    let cells: Vec<_> = s.cells().map(|(p, _)| p).collect();
    assert_eq!(cells, vec![[1, 0, 0]]);
}
