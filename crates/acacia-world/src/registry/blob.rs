//! Parser for `data/blocks.bin`; the layout is documented in `tools/gen-blocks.mjs`.

use std::sync::LazyLock;

use super::mining::{Material, Mining};
use super::offset::SelectionOffset;
use super::state::{Aabb, BlockFlags, BlockState};

static BLOB: &[u8] = include_bytes!("../../data/blocks.bin");
static SELECTION: &[u8] = include_bytes!("../../data/selection.bin");

struct Blob {
    frictions: Vec<f32>,
    strings: Vec<&'static str>,
    shapes: Vec<Box<[Aabb]>>,
    mining: Vec<Mining>,
    states_at: usize,
}

static PARSED: LazyLock<Blob> = LazyLock::new(parse_tables);

struct Reader {
    blob: &'static [u8],
    pos: usize,
}

impl Reader {
    fn bytes(&mut self, n: usize) -> &'static [u8] {
        let s = &self.blob[self.pos..self.pos + n];
        self.pos += n;
        s
    }
    /// A u8 box count, then each box as six f32 (min xyz, max xyz).
    fn boxes(&mut self) -> Box<[Aabb]> {
        (0..self.u8())
            .map(|_| {
                let v: [f32; 6] = std::array::from_fn(|_| self.f32());
                Aabb::new([v[0], v[1], v[2]], [v[3], v[4], v[5]])
            })
            .collect()
    }
    fn u8(&mut self) -> u8 {
        self.bytes(1)[0]
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.bytes(2).try_into().unwrap())
    }
    fn u32(&mut self) -> usize {
        u32::from_le_bytes(self.bytes(4).try_into().unwrap()) as usize
    }
    fn f32(&mut self) -> f32 {
        f32::from_le_bytes(self.bytes(4).try_into().unwrap())
    }
}

fn parse_tables() -> Blob {
    let mut r = Reader { blob: BLOB, pos: 0 };
    assert_eq!(r.bytes(4), b"BWB3", "blocks.bin: bad magic");
    let frictions = (0..r.u8()).map(|_| r.f32()).collect();
    let strings = (0..r.u32())
        .map(|_| {
            let n = r.u16() as usize;
            std::str::from_utf8(r.bytes(n)).expect("blocks.bin: bad utf8")
        })
        .collect();
    let shapes = (0..r.u32()).map(|_| r.boxes()).collect();
    let mining = (0..r.u16())
        .map(|_| Mining {
            hardness: r.f32(),
            material: Material::from_u8(r.u8()),
            harvest_tools: r.u8(),
            min_level: r.u8(),
        })
        .collect();
    Blob { frictions, strings, shapes, mining, states_at: r.pos }
}

/// Distinct box lists, then each state's (list index, offset byte).
type Selections = (Vec<Box<[Aabb]>>, Vec<(u16, u8)>);

/// Every vanilla state's selection boxes and offset, in runtime-id order (layout: `tools/gen-selection.py`).
fn selections() -> Vec<(&'static [Aabb], SelectionOffset)> {
    static SHAPES: LazyLock<Selections> = LazyLock::new(|| {
        let mut r = Reader { blob: SELECTION, pos: 0 };
        assert_eq!(r.bytes(4), b"BSL2", "selection.bin: bad magic");
        let shapes = (0..r.u32()).map(|_| r.boxes()).collect();
        let ids = (0..r.u32()).map(|_| (r.u16(), r.u8())).collect();
        (shapes, ids)
    });
    let (shapes, ids) = &*SHAPES;
    ids.iter().map(|&(i, o)| (&shapes[i as usize][..], SelectionOffset::from_u8(o))).collect()
}

/// Every vanilla state in runtime-id order.
pub(super) fn vanilla_states() -> Vec<BlockState> {
    let blob: &'static Blob = &PARSED;
    let selections = selections();
    let mut r = Reader { blob: BLOB, pos: blob.states_at };
    let n = r.u32();
    assert_eq!(n, selections.len(), "selection.bin is stale: regenerate it with tools/gen-selection.py");
    (0..n)
        .map(|i| {
            let name = blob.strings[r.u16() as usize];
            let properties = blob.strings[r.u16() as usize];
            let boxes = &blob.shapes[r.u16() as usize][..];
            let (selection, selection_offset) = selections[i];
            let flags = BlockFlags(r.u16());
            let liquid_depth = r.u8();
            let friction = blob.frictions[r.u8() as usize];
            let network_hash = r.u32() as u32;
            let mining = blob.mining[r.u16() as usize];
            let light = r.u8();
            BlockState {
                name,
                properties,
                boxes,
                selection,
                selection_offset,
                friction,
                flags,
                liquid_depth,
                network_hash,
                mining,
                light_emission: light >> 4,
                light_filter: light & 15,
            }
        })
        .collect()
}
