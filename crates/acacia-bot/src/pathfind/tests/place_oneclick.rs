//! The oneclick tier's blocks as the simulations' server: written from the `place_probe SET=oneclick` table
//! (2026-10-28, docs/builder/oneclick.md), apart from `build/rules/oneclick.rs` so the two can disagree.

use crate::interact::Face;

const WORDS: [&str; 6] = ["down", "up", "north", "south", "west", "east"];

fn opposite(d: &str) -> &'static str {
    let i = WORDS.iter().position(|w| *w == d).expect("direction");
    WORDS[i ^ 1]
}

fn int(d: &str) -> usize {
    WORDS.iter().position(|w| *w == d).expect("direction")
}

/// `direction` as bells and looms name it.
fn quarter(d: &str) -> usize {
    ["south", "west", "north", "east"].iter().position(|w| *w == d).expect("horizontal")
}

/// What BDS gives `n` (no namespace) clicked on `face`, `top` for a click in the upper half, `looks` the
/// player's horizontal look; `None`: not one of this tier's blocks.
pub fn bds_state(n: &str, face: Face, top: bool, looks: &str) -> Option<String> {
    let clicked = WORDS[face as usize];
    let toward = opposite(looks);
    let vertical = matches!(face, Face::Up | Face::Down);
    Some(match n {
        _ if n.ends_with("trapdoor") => {
            let d = ["east", "west", "south", "north"].iter().position(|w| *w == toward).expect("horizontal");
            format!("direction={d},open_bit=0,upside_down_bit={}", u8::from(top))
        }
        _ if n.ends_with("fence_gate") => format!("in_wall_bit=0,minecraft:cardinal_direction={toward},open_bit=0"),
        _ if n.ends_with("chest") => format!("minecraft:cardinal_direction={toward}"),
        _ if n.ends_with("anvil") => {
            let turned = match toward {
                "north" => "east",
                "east" => "south",
                "south" => "west",
                _ => "north",
            };
            format!("minecraft:cardinal_direction={turned}")
        }
        _ if n.ends_with("lantern") && n != "sea_lantern" => format!("hanging={}", u8::from(face == Face::Down)),
        _ if n.ends_with("_chain") => format!("pillar_axis={}", if vertical { "y" } else if matches!(face, Face::North | Face::South) { "z" } else { "x" }),
        "hopper" => format!("facing_direction={},toggle_bit=0", if vertical { 0 } else { int(opposite(clicked)) }),
        "end_rod" => format!("facing_direction={}", if vertical { int(clicked) } else { int(opposite(clicked)) }),
        _ if n.ends_with("lightning_rod") => format!("facing_direction={},powered_bit=0", int(clicked)),
        _ if n.ends_with("pressure_plate") => "redstone_signal=0".into(),
        "bell" | "grindstone" => {
            let (attachment, d) = match face {
                Face::Up => ("standing", toward),
                Face::Down => ("hanging", toward),
                _ => ("side", clicked),
            };
            let toggle = if n == "bell" { ",toggle_bit=0" } else { "" };
            format!("attachment={attachment},direction={}{toggle}", quarter(d))
        }
        _ if n.ends_with("glazed_terracotta") => format!("facing_direction={}", int(toward)),
        _ if n.ends_with("amethyst_cluster") || n.ends_with("amethyst_bud") => format!("minecraft:block_face={clicked}"),
        "loom" | "beehive" | "bee_nest" | "decorated_pot" | "chiseled_bookshelf" => format!("direction={}", quarter(toward)),
        _ if n.ends_with("campfire") => format!("extinguished=0,minecraft:cardinal_direction={looks}"),
        _ => return None,
    })
}
