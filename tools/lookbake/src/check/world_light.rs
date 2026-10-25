//! The Java look's world mesh and light on real terrain against the game's: tools/parity harness
//! `world_light` dumps Bedrock chunks from movement traces (`scenes.jsonl`, world_scene.rs) and
//! the game's light and quads over them, liquids included (WorldLightOracle: `light.bin`,
//! `quads.jsonl`). Here the same chunks go through acacia's light engine and mesher with the Java
//! look: every cell's light is compared, every face both draw (same corners) corner by corner
//! (block light, sky light, grey), and a face only one side draws is a divergence.

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader};
use std::path::Path;

use acacia_render::LookPack;
use acacia_render::biome::BiomeColors;
use acacia_render::light::light_now;
use acacia_render::mesh::drawn::{DrawnFace, drawn_faces};
use acacia_render::mesh::quad::LIT;
use acacia_render::mesh::{Volume, mesh_section_lit};
use acacia_world::BlockRegistry;
use serde::Deserialize;

use super::Outcome;
use super::world_scene::{Scene, block_name};
use crate::download::Error;

#[derive(Deserialize)]
struct OracleQuad {
    scene: usize,
    pos: [i32; 3],
    corners: Vec<i32>,
    light: Vec<u32>,
}

/// Per corner, in 1/256 block: where, and block light, sky light (1/16 level), grey.
type Corners = [([i32; 3], [u32; 3]); 4];

/// How far apart a corner may be: acacia stores model corners in 1/64 block.
const SLACK: i32 = 3;

fn close(a: [i32; 3], b: [i32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= SLACK)
}

fn bucket(corners: &[[i32; 3]; 4]) -> [i32; 3] {
    [0, 1, 2].map(|a| corners.iter().map(|p| p[a]).sum::<i32>().div_euclid(64))
}

/// The game's quads of one scene, by the 1/16 block their centre is in.
#[derive(Default)]
struct Theirs(HashMap<[i32; 3], Vec<([i32; 3], Corners)>>);

impl Theirs {
    fn add(&mut self, pos: [i32; 3], corners: Corners) {
        self.0.entry(bucket(&corners.map(|c| c.0))).or_default().push((pos, corners));
    }

    /// Takes the first quad at `corners` that `pick` accepts.
    fn take(&mut self, corners: &[[i32; 3]; 4], pick: impl Fn(&Corners) -> bool) -> Option<([i32; 3], Corners)> {
        let [x, y, z] = bucket(corners);
        let at = |t: &Corners| t.iter().all(|(p, _)| corners.iter().any(|q| close(*p, *q)));
        for b in (-1..=1).flat_map(|dx| (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| [x + dx, y + dy, z + dz]))) {
            let Some(group) = self.0.get_mut(&b) else { continue };
            if let Some(i) = group.iter().position(|(_, t)| at(t) && pick(t)) {
                return Some(group.swap_remove(i));
            }
        }
        None
    }
}

/// The block a face belongs to: the cell just behind its centre (faces point out of their block).
fn owner(corners: &[[i32; 3]; 4]) -> [i32; 3] {
    let [p0, p1, p2] = [0, 1, 2].map(|i| corners[i].map(|v| v as f32));
    let (a, b) = ([0, 1, 2].map(|i| p1[i] - p0[i]), [0, 1, 2].map(|i| p2[i] - p0[i]));
    let n = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let len = n.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
    [0, 1, 2].map(|i| ((corners.iter().map(|c| c[i] as f32).sum::<f32>() / 4.0 - n[i] / len) / 256.0).floor() as i32)
}

fn corners_of(face: &DrawnFace) -> Corners {
    std::array::from_fn(|k| {
        let w = face.light[k];
        (face.corners[k], [w & 0xFF, w >> 8 & 0xFF, w >> 16 & 0xFF])
    })
}

fn same(ours: &Corners, theirs: &Corners) -> bool {
    theirs.iter().all(|(p, values)| ours.iter().any(|(q, v)| close(*p, *q) && v == values))
}

#[derive(Default)]
struct Tally {
    differing: BTreeMap<String, usize>,
    ours_only: BTreeMap<String, usize>,
    theirs_only: BTreeMap<String, usize>,
}

pub fn print(look: &LookPack, scenes: &Path, quads: &Path) -> Result<Outcome, Error> {
    let mut theirs: HashMap<usize, Theirs> = HashMap::new();
    for line in BufReader::new(std::fs::File::open(quads)?).lines() {
        let q: OracleQuad = serde_json::from_str(&line?)?;
        let corners = std::array::from_fn(|k| ([0, 1, 2].map(|a| q.corners[k * 3 + a]), [0, 1, 2].map(|a| q.light[k * 3 + a])));
        theirs.entry(q.scene).or_default().add(q.pos, corners);
    }
    let their_light = std::fs::read(quads.with_file_name("light.bin"))?;
    let registry = BlockRegistry::vanilla_arc();
    let table = look.block_table(&registry);
    let (mut outcome, mut tally, mut cursor) = (Outcome::default(), Tally::default(), 0);
    for (index, line) in BufReader::new(std::fs::File::open(scenes)?).lines().enumerate() {
        let scene: Scene = serde_json::from_str(&line?)?;
        let (world, _held) = scene.build(&registry);
        let light = light_now(&world, &table);
        cursor += scene.compare_cells(&light, &their_light[cursor..], &world, &mut outcome);
        let mut faces = Vec::new();
        for (cx, cz) in scene.inner() {
            for sy in scene.min_y >> 4..(scene.min_y + scene.height as i32) >> 4 {
                let Some(v) = Volume::gather_lit(&world, cx, sy, cz, table.lighting) else { continue };
                let mesh = mesh_section_lit(&v, &table, &BiomeColors::default(), Some(&light.gather_cells((cx, sy, cz))));
                faces.extend(drawn_faces(&mesh, [cx * 16, sy * 16, cz * 16]));
            }
        }
        let mut theirs = theirs.remove(&index).unwrap_or_default();
        // Equal faces pair first, so a face only one side draws can't take another's partner.
        let mut left = Vec::new();
        for face in faces {
            let ours = corners_of(&face);
            let lit = face.light[0] & LIT != 0;
            match theirs.take(&face.corners, |t| lit && same(&ours, t)) {
                Some(_) => outcome.cases += 1,
                None => left.push(face),
            }
        }
        for face in left {
            let Some((pos, expected)) = theirs.take(&face.corners, |_| true) else {
                let pos = owner(&face.corners);
                let name = block_name(&world, pos);
                *tally.ours_only.entry(name.clone()).or_default() += 1;
                outcome.diverge(&format!("{}: {name} at {pos:?}", scene.scene), "drawn only by acacia", "no face", format!("{:?}", face.corners));
                continue;
            };
            outcome.cases += 1;
            let name = block_name(&world, pos);
            *tally.differing.entry(name.clone()).or_default() += 1;
            let show = |c: &Corners| c.iter().map(|(p, v)| format!("{p:?}={v:?}")).collect::<Vec<_>>().join(" ");
            let got = if face.light[0] & LIT == 0 { "lit from the light volume".to_owned() } else { show(&corners_of(&face)) };
            outcome.diverge(&format!("{}: {name} at {pos:?}", scene.scene), "vertex light", show(&expected), got);
        }
        for (pos, corners) in theirs.0.values().flatten() {
            let name = block_name(&world, *pos);
            *tally.theirs_only.entry(name.clone()).or_default() += 1;
            outcome.diverge(&format!("{}: {name} at {pos:?}", scene.scene), "drawn only by the game", format!("{:?}", corners.map(|c| c.0)), "no face");
        }
    }
    println!("{} cells and faces compared", outcome.cases);
    list("faces whose light differs, by block", tally.differing);
    list("faces drawn only by acacia, by block", tally.ours_only);
    list("faces drawn only by the game, by block", tally.theirs_only);
    Ok(outcome)
}

fn list(what: &str, counts: BTreeMap<String, usize>) {
    let mut counts: Vec<_> = counts.into_iter().collect();
    counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let total: usize = counts.iter().map(|(_, n)| n).sum();
    let top: Vec<String> = counts.iter().take(12).map(|(k, n)| format!("{k} {n}")).collect();
    println!("  {what}: {total} ({})", top.join(", "));
}
