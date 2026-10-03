//! Parser for `data/blocks.bin`; the layout is documented in `tools/gen-blocks.mjs`.

use std::sync::LazyLock;

use super::mining::{Material, Mining};
use super::state::{Aabb, BlockFlags, BlockState};

static BLOB: &[u8] = include_bytes!("../../data/blocks.bin");

struct Blob {
    frictions: Vec<f32>,
    strings: Vec<&'static str>,
    shapes: Vec<Box<[Aabb]>>,
    mining: Vec<Mining>,
    states_at: usize,
}

static PARSED: LazyLock<Blob> = LazyLock::new(parse_tables);

struct Reader {
    pos: usize,
}

impl Reader {
    fn bytes(&mut self, n: usize) -> &'static [u8] {
        let s = &BLOB[self.pos..self.pos + n];
        self.pos += n;
        s
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
    let mut r = Reader { pos: 0 };
    assert_eq!(r.bytes(4), b"BWB2", "blocks.bin: bad magic");
    let frictions = (0..r.u8()).map(|_| r.f32()).collect();
    let strings = (0..r.u32())
        .map(|_| {
            let n = r.u16() as usize;
            std::str::from_utf8(r.bytes(n)).expect("blocks.bin: bad utf8")
        })
        .collect();
    let shapes = (0..r.u32())
        .map(|_| {
            (0..r.u8())
                .map(|_| {
                    let v: [f32; 6] = std::array::from_fn(|_| r.f32());
                    Aabb::new([v[0], v[1], v[2]], [v[3], v[4], v[5]])
                })
                .collect()
        })
        .collect();
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

/// Every vanilla state in runtime-id order.
pub(super) fn vanilla_states() -> Vec<BlockState> {
    let blob: &'static Blob = &PARSED;
    let mut r = Reader { pos: blob.states_at };
    (0..r.u32())
        .map(|_| {
            let name = blob.strings[r.u16() as usize];
            let properties = blob.strings[r.u16() as usize];
            let boxes = &blob.shapes[r.u16() as usize][..];
            let flags = BlockFlags(r.u16());
            let liquid_depth = r.u8();
            let friction = blob.frictions[r.u8() as usize];
            let network_hash = r.u32() as u32;
            let mining = blob.mining[r.u16() as usize];
            BlockState {
                name,
                properties,
                boxes,
                friction,
                flags,
                liquid_depth,
                network_hash,
                mining,
            }
        })
        .collect()
}
