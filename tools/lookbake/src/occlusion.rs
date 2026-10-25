//! What a block is to its neighbours' faces in Java ([`Cull`], `Block.shouldRenderFace`): the
//! face occlusion shapes the game builds in code, written by `BlockShapeExporter`, and
//! `skipRendering` by block class. Checked against the game by `lookbake java-light-check`
//! (tools/parity harness `light_facts`).

use std::collections::HashMap;
use std::path::Path;

use acacia_render::blocks::{Cull, FaceShapes, Skip};
use serde::Deserialize;

use crate::download::Error;
use crate::exporter::Exporter;
use crate::mapping::JavaState;

#[derive(Deserialize)]
struct Occluding {
    faces: Vec<String>,
    whole: Vec<bool>,
}

/// The exporter's shapes of every state that can occlude (`canOcclude`), by [`JavaState::key`].
pub struct Shapes(HashMap<String, Occluding>);

impl Shapes {
    /// `None` without a JDK 25: the look then keeps the renderer's own culling.
    pub fn export(java_dir: &Path) -> Result<Option<Shapes>, Error> {
        let Some(exporter) = Exporter::prepare(java_dir)? else { return Ok(None) };
        let out = exporter.output("BlockShapeExporter")?;
        Ok(Some(Shapes(serde_json::from_slice(&std::fs::read(out)?)?)))
    }
}

/// `BlockShapeExporter.grid`'s hex: bit `i` is digit `i / 4`'s bit `3 - i % 4`.
pub fn grid(hex: &str) -> [u64; 4] {
    let mut words = [0u64; 4];
    for (i, digit) in hex.chars().enumerate() {
        let nibble = u64::from(digit.to_digit(16).unwrap_or(0));
        for b in 0..4 {
            if nibble >> (3 - b) & 1 == 1 {
                let bit = i * 4 + b;
                words[bit / 64] |= 1 << (bit % 64);
            }
        }
    }
    words
}

/// `skipRendering`'s rule, by block class.
fn skip(java: &JavaState, block: u16) -> Skip {
    let name = java.name.as_str();
    match () {
        _ if name.ends_with("bars") || name.ends_with("glass_pane") => {
            let sides = ["north", "south", "west", "east"].iter().enumerate().filter(|(_, d)| java.property(d) == Some("true")).fold(0, |s, (i, _)| s | 1 << (i + 2));
            Skip::Bars { block, tagged: name.ends_with("bars"), sides }
        }
        _ if name.ends_with("glass") || name.ends_with("grate") || matches!(name, "ice" | "blue_ice" | "frosted_ice" | "honey_block" | "slime_block" | "powder_snow") => Skip::Same { block },
        _ if name == "mangrove_roots" => Skip::SameVertical { block },
        _ => Skip::None,
    }
}

/// Java block names numbered for [`Skip`]'s `block`.
#[derive(Default)]
pub struct BlockNumbers(HashMap<String, u16>);

impl BlockNumbers {
    fn of(&mut self, name: &str) -> u16 {
        let next = u16::try_from(self.0.len()).expect("Java blocks fit a u16");
        *self.0.entry(name.to_owned()).or_insert(next)
    }
}

/// `shape` interns face shapes as [`Cull::shape`].
pub fn cull(shapes: &Shapes, java: &JavaState, numbers: &mut BlockNumbers, shape: impl FnOnce(FaceShapes) -> u16) -> Cull {
    let skip = skip(java, numbers.of(&java.name));
    match shapes.0.get(&java.key()) {
        Some(o) => Cull { shape: shape(std::array::from_fn(|d| grid(&o.faces[d]))), whole: o.whole.iter().rev().fold(0, |w, &b| w << 1 | u8::from(b)), skip },
        None => Cull { shape: 0, whole: 0, skip },
    }
}
