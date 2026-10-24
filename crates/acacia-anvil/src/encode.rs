//! A converted column as Java 26.3 chunk NBT (the uncompressed payload of a region slot).

use crate::column::JavaColumn;
use crate::light::SectionLight;
use crate::mapping::{self, JavaState};
use crate::nbt::{self, NbtWriter};
use crate::pack::{biome_bits, block_bits, pack_into};
use crate::props::{self, PropsTable};
use crate::section::{Blocks, VOLUME};

/// `world_version` of Java 26.3, the version the state table comes from.
pub const DATA_VERSION: i32 = 5023;

/// A block entity as BlueMap reads it (`id` and position); rendering takes everything else from states.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockEntity {
    pub id: &'static str,
    pub pos: [i32; 3],
}

#[derive(Default)]
pub struct EncodeScratch {
    /// Palette slot + 1 per Java state, reset after each section.
    slot: Vec<u16>,
    palette: Vec<JavaState>,
    indices: Vec<u16>,
    longs: Vec<u64>,
}

const HEIGHTMAPS: [(&str, u8, u8); 4] = [
    ("WORLD_SURFACE", props::NON_AIR, 0),
    ("OCEAN_FLOOR", props::BLOCKS_MOTION, 0),
    ("MOTION_BLOCKING", props::BLOCKS_MOTION | props::FLUID, 0),
    (
        "MOTION_BLOCKING_NO_LEAVES",
        props::BLOCKS_MOTION | props::FLUID,
        props::LEAVES,
    ),
];

pub fn encode_chunk(
    col: &JavaColumn,
    light: &[SectionLight],
    block_entities: &[BlockEntity],
    props: &PropsTable,
    s: &mut EncodeScratch,
    out: &mut Vec<u8>,
) {
    out.clear();
    let mut w = NbtWriter::root(out);
    w.int("DataVersion", DATA_VERSION);
    w.string("Status", "minecraft:full");
    w.int("xPos", col.x);
    w.int("zPos", col.z);
    w.int("yPos", col.min_section);
    w.long("LastUpdate", 0);
    w.long("InhabitedTime", 0);
    w.byte("isLightOn", 1);
    w.list("sections", nbt::COMPOUND, col.sections.len());
    for (i, l) in light.iter().enumerate().take(col.sections.len()) {
        write_section(&mut w, col, i, l, s);
    }
    w.list("block_entities", nbt::COMPOUND, block_entities.len());
    for be in block_entities {
        w.string("id", be.id);
        w.int("x", be.pos[0]);
        w.int("y", be.pos[1]);
        w.int("z", be.pos[2]);
        w.byte("keepPacked", 0);
        w.end();
    }
    w.compound("Heightmaps");
    for (name, want, exclude) in HEIGHTMAPS {
        heightmap(col, props, want, exclude, s);
        w.long_array(name, &s.longs);
    }
    w.end();
    w.end();
}

fn write_section(
    w: &mut NbtWriter,
    col: &JavaColumn,
    i: usize,
    light: &SectionLight,
    s: &mut EncodeScratch,
) {
    let j = mapping::java();
    w.byte("Y", (col.min_section + i as i32) as i8);

    build_palette(s, &col.sections[i], j.state_count());
    w.compound("block_states");
    w.list("palette", nbt::COMPOUND, s.palette.len());
    for &state in &s.palette {
        write_state(w, state);
    }
    if s.palette.len() > 1 {
        pack_into(&s.indices, block_bits(s.palette.len()), &mut s.longs);
        w.long_array("data", &s.longs);
    }
    w.end();

    let biomes = mapping::biomes();
    let cells = &col.biomes[i];
    let mut palette: Vec<u16> = Vec::new();
    s.indices.clear();
    for &b in cells {
        let p = palette.iter().position(|&q| q == b).unwrap_or_else(|| {
            palette.push(b);
            palette.len() - 1
        });
        s.indices.push(p as u16);
    }
    w.compound("biomes");
    w.list("palette", nbt::STRING, palette.len());
    for &b in &palette {
        w.list_string(biomes.name(b));
    }
    if palette.len() > 1 {
        pack_into(&s.indices, biome_bits(palette.len()), &mut s.longs);
        w.long_array("data", &s.longs);
    }
    w.end();

    w.byte_array("BlockLight", &light.block[..]);
    w.byte_array("SkyLight", &light.sky[..]);
    w.end();
}

/// A palette entry in 26.3's form: compounds `{id, properties}` (Java's own writer drops to a bare name
/// for default states, which readers resolve the same way; always spelling the properties out is safe).
fn write_state(w: &mut NbtWriter, state: JavaState) {
    let j = mapping::java();
    w.string("id", j.block(state).name);
    if j.properties(state).next().is_some() {
        w.compound("properties");
        for (k, v) in j.properties(state) {
            w.string(k, v);
        }
        w.end();
    }
    w.end();
}

/// The section's palette in first-occurrence order and each block's index into it (left stale for a
/// one-entry palette, which writes no data).
fn build_palette(s: &mut EncodeScratch, blocks: &Blocks, state_count: usize) {
    if s.slot.len() != state_count {
        s.slot = vec![0; state_count];
    }
    s.palette.clear();
    s.indices.resize(VOLUME, 0);
    let (slots, palette) = (&mut s.slot, &mut s.palette);
    let mut slot_of = |st: JavaState| {
        let slot = &mut slots[st as usize];
        if *slot == 0 {
            palette.push(st);
            *slot = palette.len() as u16;
        }
        *slot - 1
    };
    match blocks {
        Blocks::Uniform(st) => {
            slot_of(*st);
        }
        // Mapped per small index, so a section's own palette (even with repeats) costs one lookup each.
        Blocks::Small { palette: small, idx } => {
            let mut map = [u16::MAX; 256];
            for (o, &i) in s.indices.iter_mut().zip(idx.iter()) {
                let m = &mut map[i as usize];
                if *m == u16::MAX {
                    *m = slot_of(small[i as usize]);
                }
                *o = *m;
            }
        }
        Blocks::Full(states) => {
            for (o, &st) in s.indices.iter_mut().zip(states.iter()) {
                *o = slot_of(st);
            }
        }
    }
    for &st in &s.palette {
        s.slot[st as usize] = 0;
    }
}

/// Java's heightmap: per column (index `z * 16 + x`) one above the highest matching block, relative to
/// the bottom of the world, packed at `ceil(log2(height + 1))` bits.
fn heightmap(col: &JavaColumn, props: &PropsTable, want: u8, exclude: u8, s: &mut EncodeScratch) {
    let (min_y, height) = (col.min_y(), col.height());
    let top = col
        .sections
        .iter()
        .rposition(|b| b.uniform() != Some(mapping::AIR))
        .map_or(min_y, |t| min_y + (t as i32 + 1) * 16);
    s.indices.clear();
    for z in 0..16 {
        for x in 0..16 {
            let mut h = 0;
            for y in (min_y..top).rev() {
                let f = props.get(col.block(x, y, z)).flags;
                if f & want != 0 && f & exclude == 0 {
                    h = y - min_y + 1;
                    break;
                }
            }
            s.indices.push(h as u16);
        }
    }
    let bits = u32::BITS - (height as u32).leading_zeros();
    pack_into(&s.indices, bits, &mut s.longs);
}
