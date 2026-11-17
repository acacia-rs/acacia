//! Block dumps around a replayed position, for triaging a mismatch.

use acacia_physics::Vec3;
use acacia_world::BlockAccess;

use crate::world::WorldTracker;

/// Distinct non-air block names in the player's box (widened by 0.1) and the layer below its feet.
pub(super) fn nearby_blocks(world: &WorldTracker, [x, y, z]: Vec3) -> String {
    let (Some(view), Some(registry)) = (world.view(), world.registry()) else { return String::new() };
    let span = |lo: f32, hi: f32| lo.floor() as i32..=hi.floor() as i32;
    let mut names: Vec<&str> = Vec::new();
    for bx in span(x - 0.4, x + 0.4) {
        for by in span(y - 1.0, y + 1.9) {
            for bz in span(z - 0.4, z + 0.4) {
                for id in [view.block(bx, by, bz), view.liquid(bx, by, bz)] {
                    if let Some(s) = registry.get(id).filter(|s| !s.is_air()) {
                        let name = s.name.trim_start_matches("minecraft:");
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
            }
        }
    }
    // Liquid levels (Bedrock liquid_depth) in the 5x5 around the feet, at and below their layer.
    let (fx, fy, fz) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    let mut liquids = Vec::new();
    for by in [fy, fy - 1] {
        for bz in fz - 2..=fz + 2 {
            for bx in fx - 2..=fx + 2 {
                let state = [(view.block(bx, by, bz), ""), (view.liquid(bx, by, bz), "L")]
                    .into_iter()
                    .filter_map(|(id, layer)| registry.get(id).map(|s| (s, layer)))
                    .find(|(s, _)| s.is_water());
                // `L` marks water in the second (liquid) layer.
                if let Some((s, layer)) = state {
                    liquids.push(format!("{},{},{}:{}{layer}", bx - fx, by - fy, bz - fz, s.liquid_depth));
                }
            }
        }
    }
    // Blocks other than stone in the 3x3 below the feet (surface effects) and at the feet (thin blocks).
    let layer = |by: i32| {
        let mut found = Vec::new();
        for bz in fz - 1..=fz + 1 {
            for bx in fx - 1..=fx + 1 {
                if let Some(s) = registry.get(view.block(bx, by, bz)).filter(|s| !s.is_air() && !s.name.ends_with(":stone")) {
                    found.push(format!("{},{}:{}", bx - fx, bz - fz, s.name.trim_start_matches("minecraft:")));
                }
            }
        }
        found
    };
    let mut out = names.join(",");
    for (label, found) in [("floor", layer(fy - 1)), ("feet", layer(fy))] {
        if !found.is_empty() {
            out += &format!(" {label} {}", found.join(" "));
        }
    }
    if !liquids.is_empty() {
        out += &format!(" water {}", liquids.join(" "));
    }
    // Collision boxes of non-stone blocks in the 3x3 around the feet, feet to head layer.
    if std::env::var_os("BTRC_BOXES").is_some() {
        for by in fy..=fy + 2 {
            for bz in fz - 1..=fz + 1 {
                for bx in fx - 1..=fx + 1 {
                    let Some(s) = registry.get(view.block(bx, by, bz)).filter(|s| !s.boxes.is_empty() && !s.name.ends_with(":stone")) else { continue };
                    let boxes: Vec<_> = s.boxes.iter().map(|b| (b.min, b.max)).collect();
                    out += &format!("\n  [{bx},{by},{bz}] {} {boxes:?}", s.name);
                }
            }
        }
    }
    if std::env::var_os("BTRC_MAP").is_some() {
        out += &block_map(view, registry, [fx, fy, fz]);
    }
    out
}

/// Layers feet-1..=feet+2 of the 5x5 around the feet (x across, z down): `.` air, `#` full block, `~` water
/// (with its depth), else the first letter of the name.
fn block_map(view: &impl BlockAccess, registry: &acacia_world::BlockRegistry, [fx, fy, fz]: [i32; 3]) -> String {
    let mut out = String::new();
    for by in fy - 1..=fy + 2 {
        out += &format!("\n  y{:+}", by - fy);
        for bz in fz - 2..=fz + 2 {
            out += " ";
            for bx in fx - 2..=fx + 2 {
                let solid = registry.get(view.block(bx, by, bz)).filter(|s| !s.is_air());
                let water = [view.block(bx, by, bz), view.liquid(bx, by, bz)].into_iter().filter_map(|id| registry.get(id)).find(|s| s.is_water());
                out += &match (solid, water) {
                    (Some(s), _) if !s.is_water() => {
                        let n = s.name.trim_start_matches("minecraft:");
                        if n == "stone" { "#".into() } else { n[..1].to_string() }
                    }
                    (_, Some(w)) => format!("{}", w.liquid_depth % 10),
                    _ => ".".into(),
                };
            }
        }
    }
    out
}
