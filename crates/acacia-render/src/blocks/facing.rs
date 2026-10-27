//! Which way a block faces: logs' axis, cardinal fronts, and the six-way blocks (pistons, dispensers,
//! droppers, observers). blocks.json cannot place a six-way block's faces (the game hardcodes them):
//! they are laid out here as Java's models do, front north, then turned to the block's facing.

use acacia_world::BlockState;

/// Face order as `RenderBlock::textures`: east, west, up, down, south, north.
const NORMALS: [[i8; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
const UP: [i8; 3] = [0, 1, 0];
const NORTH: [i8; 3] = [0, 0, -1];
const SOUTH: [i8; 3] = [0, 0, 1];
/// Where a texture's top edge points on each face before any turn: up on the sides, north on the top
/// and bottom.
const TOP_EDGE: [[i8; 3]; 6] = [UP, UP, NORTH, NORTH, UP, UP];

/// Applies `pillar_axis`, `minecraft:cardinal_direction` (blocks.json fronts face south) and six-way
/// facing to the pack's face names.
pub(super) fn orient(mut f: [String; 6], state: &BlockState) -> [String; 6] {
    if let Some(layout) = six_way(state) {
        return layout.map(|(name, _)| name);
    }
    let (top, bottom, side) = (f[2].clone(), f[3].clone(), f[4].clone());
    match state.property("pillar_axis") {
        Some("x") => f = [top.clone(), bottom, side.clone(), side.clone(), side.clone(), side],
        Some("z") => f = [side.clone(), side.clone(), side.clone(), side, top.clone(), bottom],
        _ => {}
    }
    let turns = match state.property("minecraft:cardinal_direction") {
        Some("west") => 1,
        Some("north") => 2,
        Some("east") => 3,
        _ => 0,
    };
    // South, west, north, east: each turn moves a face one step along this ring.
    const RING: [usize; 4] = [4, 1, 5, 0];
    let old = f.clone();
    for i in 0..4 {
        f[RING[(i + turns) % 4]] = old[RING[i]].clone();
    }
    f
}

/// Quarter turns of each face's texture.
pub(super) fn turns(state: &BlockState) -> [u8; 6] {
    six_way(state).map_or([0; 6], |layout| std::array::from_fn(|i| quarter_turns(i, layout[i].1)))
}

/// Each face's texture and where its top edge points, for a six-way block.
fn six_way(state: &BlockState) -> Option<[(String, [i8; 3]); 6]> {
    let name = super::short_name(state.name);
    // `facing_direction` 0..=5 is down, up, north, south, west, east; `order` maps it to a face.
    let facing = |order: [usize; 6]| Some(NORMALS[order[state.property("facing_direction")?.parse::<usize>().ok()?.min(5)]]);
    let (front, base): ([i8; 3], [(String, [i8; 3]); 6]) = match name {
        // Bedrock numbers a piston's horizontal facings the other way round (ViaBedrock's table).
        "piston" | "sticky_piston" => {
            let front = facing([3, 2, 4, 5, 0, 1])?;
            let platform = if name == "piston" { "piston_top_normal" } else { "piston_top_sticky" };
            let side = || ("piston_side".to_owned(), NORTH);
            (front, [side(), side(), side(), side(), ("piston_bottom".into(), UP), (platform.into(), UP)])
        }
        "dispenser" | "dropper" => {
            let front = facing([3, 2, 5, 4, 1, 0])?;
            let [top, side] = [format!("{name}_top"), format!("{name}_side")];
            let (face, rest) = match front[1] {
                0 => (format!("{name}_front_horizontal"), side),
                _ => (format!("{name}_front_vertical"), top.clone()),
            };
            let s = |t: &String| (t.clone(), UP);
            (front, [s(&rest), s(&rest), (top.clone(), NORTH), (top, NORTH), s(&rest), s(&face)])
        }
        "observer" => {
            let front = match state.property("minecraft:facing_direction")? {
                "east" => 0,
                "west" => 1,
                "up" => 2,
                "down" => 3,
                "south" => 4,
                _ => 5,
            };
            let s = |t: &str, up| (t.to_owned(), up);
            // The arrow points away from the face, at the output (Java's model flips the up face's uv).
            let top = || s("observer_top", SOUTH);
            (NORMALS[front], [s("observer_east", UP), s("observer_west", UP), top(), top(), s("observer_south", UP), s("observer_north", UP)])
        }
        _ => return None,
    };
    let mut out = base.clone();
    for (i, (texture, edge)) in base.into_iter().enumerate() {
        let to = NORMALS.iter().position(|&n| n == turn(front, NORMALS[i])).expect("a turned normal is a normal");
        out[to] = (texture, turn(front, edge));
    }
    Some(out)
}

/// The turn taking north to `front`, applied to `v` (Java's blockstate y and x rotations).
fn turn(front: [i8; 3], [x, y, z]: [i8; 3]) -> [i8; 3] {
    match front {
        [1, 0, 0] => [-z, y, x],
        [-1, 0, 0] => [z, y, -x],
        [0, 0, 1] => [-x, y, -z],
        [0, 1, 0] => [x, -z, y],
        [0, -1, 0] => [x, z, -y],
        _ => [x, y, z],
    }
}

/// Quarter turns about face `face`'s normal taking its default top edge to `edge`, clockwise seen from
/// outside (the mesher's sense: seen live on pistons' side bands).
fn quarter_turns(face: usize, edge: [i8; 3]) -> u8 {
    let n = NORMALS[face];
    let counter_clockwise = |[a, b, c]: [i8; 3]| [n[1] * c - n[2] * b, n[2] * a - n[0] * c, n[0] * b - n[1] * a];
    let mut v = TOP_EDGE[face];
    for k in 0..4 {
        if v == edge {
            return (4 - k) % 4;
        }
        v = counter_clockwise(v);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(name: &str, props: &str) -> &'static BlockState {
        let reg = acacia_world::BlockRegistry::vanilla();
        let id = reg.find(name, props).unwrap_or_else(|| panic!("no state {name} {props}"));
        reg.get(id).unwrap()
    }

    fn faces(name: &str, props: &str) -> [String; 6] {
        orient(std::array::from_fn(|_| String::new()), state(name, props))
    }

    #[test]
    fn a_pistons_platform_is_on_its_facing_and_bedrock_swaps_horizontals() {
        assert_eq!(faces("minecraft:piston", "facing_direction=1")[2], "piston_top_normal");
        assert_eq!(faces("minecraft:piston", "facing_direction=1")[3], "piston_bottom");
        assert_eq!(faces("minecraft:sticky_piston", "facing_direction=2")[4], "piston_top_sticky");
        assert_eq!(faces("minecraft:piston", "facing_direction=4")[0], "piston_top_normal");
    }

    #[test]
    fn a_pistons_sides_point_their_band_at_the_platform() {
        let up = turns(state("minecraft:piston", "facing_direction=1"));
        assert_eq!([up[0], up[1], up[4], up[5]], [0; 4], "sides of an upward piston stand upright");
        let north = turns(state("minecraft:piston", "facing_direction=3"));
        assert_eq!(north[2], 0, "the top's band points north as drawn");
        assert_ne!(north[0], 0);
    }

    #[test]
    fn dispensers_show_the_vertical_front_only_facing_up_or_down() {
        let up = faces("minecraft:dispenser", "facing_direction=1,triggered_bit=0");
        assert_eq!(up[2], "dispenser_front_vertical");
        assert_eq!(up[0], "dispenser_top");
        let west = faces("minecraft:dropper", "facing_direction=4,triggered_bit=0");
        assert_eq!(west[1], "dropper_front_horizontal");
        assert_eq!([&west[0], &west[2]], ["dropper_side", "dropper_top"]);
    }

    #[test]
    fn an_observers_face_looks_its_facing() {
        let down = faces("minecraft:observer", "minecraft:facing_direction=down,powered_bit=0");
        assert_eq!([&down[3], &down[2]], ["observer_north", "observer_south"]);
        assert_eq!(faces("minecraft:observer", "minecraft:facing_direction=east,powered_bit=0")[0], "observer_north");
    }

    #[test]
    fn an_observers_arrow_points_at_its_back() {
        let north = turns(state("minecraft:observer", "minecraft:facing_direction=north,powered_bit=0"));
        assert_eq!([north[2], north[3]], [2, 2]);
        let east = turns(state("minecraft:observer", "minecraft:facing_direction=east,powered_bit=0"));
        assert_eq!(east[2], 3, "three clockwise quarter turns seen from above point it west");
    }

    #[test]
    fn other_blocks_keep_the_packs_faces() {
        assert!(turns(state("minecraft:furnace", "minecraft:cardinal_direction=north")).iter().all(|&t| t == 0));
    }
}
