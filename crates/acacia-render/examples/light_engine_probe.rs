//! Parity probe for harness `light_engine` (docs/parity.md): lights the oracle's layouts with the
//! engine under Java's rules, applies their edits block by block, and compares every cell's block
//! and sky light. Usage: `light_engine_probe <layouts.jsonl>`; prints CASES and DIVERGENCE lines.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};

use acacia_render::blocks::FaceShapes;
use acacia_render::light::LightData;
use acacia_render::light::blocks::{Cell, LightBlocks};
use acacia_render::light::column::Lighter;
use acacia_render::light::data::props;
use acacia_render::light::rules::Rules;
use acacia_world::{Dimension, SECTION_VOLUME};
use serde_json::Value;

/// Divergent cells shown per case and channel.
const SHOWN: usize = 3;

struct Layout {
    chunks: [i32; 2],
    height: i32,
    cells: HashMap<[i32; 3], Cell>,
}

impl LightBlocks for Layout {
    fn column(&self, cx: i32, cz: i32) -> Option<Vec<Option<Box<[Cell; SECTION_VOLUME]>>>> {
        if !(0..self.chunks[0]).contains(&cx) || !(0..self.chunks[1]).contains(&cz) {
            return None;
        }
        let section = |sy: i32| {
            let mut cells = Box::new([Cell::default(); SECTION_VOLUME]);
            let mut any = false;
            for (&[x, y, z], cell) in &self.cells {
                if x >> 4 == cx && z >> 4 == cz && y >> 4 == sy {
                    cells[(((x & 15) << 8) | ((z & 15) << 4) | (y & 15)) as usize] = *cell;
                    any = true;
                }
            }
            any.then_some(cells)
        };
        Some((0..self.height / 16).map(section).collect())
    }

    fn cell(&self, x: i32, y: i32, z: i32) -> Option<Cell> {
        let loaded = (0..self.chunks[0]).contains(&(x >> 4)) && (0..self.chunks[1]).contains(&(z >> 4));
        loaded.then(|| self.cells.get(&[x, y, z]).copied().unwrap_or_default())
    }

    fn air(&self) -> Cell {
        Cell::default()
    }
}

fn faces(v: &Value) -> FaceShapes {
    std::array::from_fn(|d| {
        let hex = v[d].as_str().unwrap().as_bytes();
        let mut words = [0u64; 4];
        for i in 0..256 {
            let nibble = (hex[i / 4] as char).to_digit(16).unwrap();
            if nibble >> (3 - i % 4) & 1 == 1 {
                words[i / 64] |= 1 << (i % 64);
            }
        }
        words
    })
}

fn place(v: &Value) -> ([i32; 3], &str) {
    let p = |i: usize| v[i].as_i64().unwrap() as i32;
    ([p(0), p(1), p(2)], v[3].as_str().unwrap())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = BufReader::new(std::fs::File::open(&args[1]).expect("layouts"));
    let (mut cases, mut cells_compared, mut cells_wrong) = (0, 0usize, 0usize);
    for line in input.lines().map(Result::unwrap).filter(|l| !l.trim().is_empty()) {
        let row: Value = serde_json::from_str(&line).expect("layout");
        let case = row["case"].as_str().unwrap();
        let mut shapes: Vec<FaceShapes> = Vec::new();
        let mut cell_of: HashMap<&str, Cell> = HashMap::new();
        for (state, f) in row["states"].as_object().unwrap() {
            let level = |k: &str| f[k].as_u64().unwrap() as u8;
            let shape = match f.get("faces") {
                Some(v) => {
                    shapes.push(faces(v));
                    shapes.len() as u8
                }
                None => 0,
            };
            cell_of.insert(state, Cell { props: props(level("emission"), level("dampening")), shape });
        }
        let chunks = [row["chunks"][0].as_i64().unwrap() as i32, row["chunks"][1].as_i64().unwrap() as i32];
        let height = row["height"].as_i64().unwrap() as i32;
        let cells = row["blocks"].as_array().unwrap().iter().map(place).map(|(p, s)| (p, cell_of[s])).collect();
        let dim = Dimension { min_y: 0, height: height as u32, air: 0, sky: row["sky"].as_bool().unwrap() };
        let mut lighter = Lighter { blocks: Layout { chunks, height, cells }, rules: Rules::Java(&shapes), dim };
        let mut data = LightData::default();
        for cx in 0..chunks[0] {
            for cz in 0..chunks[1] {
                lighter.relight_column(&mut data, cx, cz);
            }
        }
        for edit in row["edits"].as_array().unwrap() {
            let (p, s) = place(edit);
            lighter.blocks.cells.insert(p, cell_of[s]);
            lighter.update_block(&mut data, p[0], p[1], p[2]);
        }
        cases += 1;
        for (channel, key, shift) in [("block", "block_light", 4), ("sky", "sky_light", 0)] {
            let expected = row[key].as_str().unwrap().as_bytes();
            let mut i = 0;
            let mut shown = 0;
            for x in 0..chunks[0] * 16 {
                for z in 0..chunks[1] * 16 {
                    for y in 0..height {
                        let want = (expected[i] as char).to_digit(16).unwrap() as u8;
                        let got = data.light(x, y, z).map_or(0, |l| l >> shift & 15);
                        cells_compared += 1;
                        if want != got {
                            cells_wrong += 1;
                            if shown < SHOWN {
                                println!("DIVERGENCE\t{case}\t{channel}@{x},{y},{z}\t{want}\t{got}");
                                shown += 1;
                            }
                        }
                        i += 1;
                    }
                }
            }
        }
    }
    println!("CASES\t{cases}");
    eprintln!("{cells_wrong} of {cells_compared} cells differ");
}
