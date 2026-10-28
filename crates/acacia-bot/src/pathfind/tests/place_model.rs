//! How BDS sets a placed block's state, as the simulations' server: written from the place_probe table
//! (2026-10-17), apart from `build/rules.rs` so the two can disagree.

use acacia_physics::{BlockPos, Vec3};

use crate::interact::Face;

fn word(face: Face) -> &'static str {
    ["down", "up", "north", "south", "west", "east"][face as usize]
}

/// The state BDS gives `item` placed by this click; `None`: refused.
pub fn bds_state(item: &str, face: Face, click: Vec3, eye: Vec3, point: Vec3, cell: &str, pos: BlockPos) -> Option<(String, String)> {
    let n = item.trim_start_matches("minecraft:");
    let top = match face {
        Face::Down => true,
        Face::Up => false,
        _ => click[1] > 0.5,
    };
    let (dx, dz) = (point[0] - eye[0], point[2] - eye[2]);
    let looks = if dx.abs() > dz.abs() { if dx > 0.0 { "east" } else { "west" } } else if dz > 0.0 { "south" } else { "north" };
    let opposite = |d: &str| match d {
        "north" => "south",
        "south" => "north",
        "east" => "west",
        "west" => "east",
        "up" => "down",
        _ => "up",
    };
    if let Some(props) = super::place_oneclick::bds_state(n, face, top, looks) {
        return Some((n.to_string(), props));
    }
    let props = match n {
        _ if n.ends_with("_slab") && cell == format!("minecraft:{n}") => return Some((n.replace("_slab", "_double_slab"), "minecraft:vertical_half=bottom".into())),
        _ if n.ends_with("_slab") => format!("minecraft:vertical_half={}", if top { "top" } else { "bottom" }),
        _ if n.ends_with("_log") => format!("pillar_axis={}", match face {
            Face::Up | Face::Down => "y",
            Face::North | Face::South => "z",
            _ => "x",
        }),
        _ if n.ends_with("_stairs") => {
            let weirdo = match looks {
                "east" => 0,
                "west" => 1,
                "south" => 2,
                _ => 3,
            };
            format!("upside_down_bit={},weirdo_direction={weirdo}", u8::from(top))
        }
        "torch" | "redstone_torch" => match face {
            Face::Down => return None,
            Face::Up => "torch_facing_direction=top".into(),
            f => format!("torch_facing_direction={}", opposite(word(f))),
        },
        "furnace" => format!("minecraft:cardinal_direction={}", opposite(looks)),
        "ladder" if matches!(face, Face::Up | Face::Down) => return None,
        "ladder" | "stone_button" => format!("facing_direction={}", face as u8),
        "observer" | "piston" | "dispenser" => {
            // Vertical within a block sideways with the eye over 2 above the block or below it (probe E1-high/low).
            let (above, off) = (eye[1] - pos[1] as f32, (eye[0] - pos[0] as f32 - 0.5).abs().max((eye[2] - pos[2] as f32 - 0.5).abs()));
            let toward = match () {
                _ if off < 1.5 && above > 2.0 => "up",
                _ if off < 1.5 && above < 0.0 => "down",
                _ => opposite(looks),
            };
            let away = |d: &'static str| if n == "dispenser" || (n == "piston" && matches!(d, "up" | "down")) { d } else { opposite(d) };
            let d = away(toward);
            let int = ["down", "up", "north", "south", "west", "east"].iter().position(|w| *w == d).expect("direction");
            if n == "observer" { format!("minecraft:facing_direction={d}") } else { format!("facing_direction={int}") }
        }
        _ => String::new(),
    };
    Some((n.to_string(), props))
}
