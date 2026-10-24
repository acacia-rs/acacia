//! Java properties Bedrock does not carry, rebuilt from neighbours the way Java's `updateShape` does
//! (the full list of what Geyser's mapping loses: data/ambiguous.txt). Rules only read properties no rule
//! writes, so the pass gives the same result in any order.

use crate::light::Neighbourhood;
use crate::mapping::{self, JavaState};
use crate::props::PropsTable;
use crate::section::Blocks;

#[derive(Clone, Copy, PartialEq)]
enum Rule {
    None,
    /// grass_block, podzol, mycelium: `snowy` when snow is on top.
    Snowy,
    /// Huge mushroom blocks: a face against the same block is not drawn.
    Mushroom,
    /// Kelp and nether/cave vines: the end block (`head`) vs the rest (`body`), by the next block along
    /// the growth direction.
    Plant {
        family: u8,
        up: bool,
        body: bool,
    },
    RedstoneWire,
    /// Pistons: `extended` when their head is in front.
    Piston,
}

/// (head, body, grows up)
const PLANTS: [(&str, &str, bool); 4] = [
    ("minecraft:kelp", "minecraft:kelp_plant", true),
    (
        "minecraft:twisting_vines",
        "minecraft:twisting_vines_plant",
        true,
    ),
    (
        "minecraft:weeping_vines",
        "minecraft:weeping_vines_plant",
        false,
    ),
    ("minecraft:cave_vines", "minecraft:cave_vines_plant", false),
];

const SIDES: [(&str, i32, i32); 4] = [
    ("north", 0, -1),
    ("south", 0, 1),
    ("west", -1, 0),
    ("east", 1, 0),
];

pub struct Fixups {
    rules: Vec<Rule>,
    names: Vec<&'static str>,
}

impl Default for Fixups {
    fn default() -> Self {
        Self::new()
    }
}

impl Fixups {
    pub fn new() -> Self {
        let j = mapping::java();
        let mut rules = Vec::with_capacity(j.state_count());
        let mut names = Vec::with_capacity(j.state_count());
        for s in 0..j.state_count() {
            let name = j.block(s as JavaState).name;
            names.push(name);
            let plant = PLANTS.iter().enumerate().find_map(|(i, (head, body, up))| {
                (name == *head || name == *body).then_some(Rule::Plant {
                    family: i as u8,
                    up: *up,
                    body: name == *body,
                })
            });
            rules.push(match name {
                "minecraft:grass_block" | "minecraft:podzol" | "minecraft:mycelium" => Rule::Snowy,
                "minecraft:brown_mushroom_block"
                | "minecraft:red_mushroom_block"
                | "minecraft:mushroom_stem" => Rule::Mushroom,
                "minecraft:redstone_wire" => Rule::RedstoneWire,
                "minecraft:piston" | "minecraft:sticky_piston" => Rule::Piston,
                _ => plant.unwrap_or(Rule::None),
            });
        }
        Self { rules, names }
    }

    fn has_rule(&self, b: &Blocks) -> bool {
        match b {
            Blocks::Uniform(s) => self.rules[*s as usize] != Rule::None,
            Blocks::Small { palette, .. } => palette
                .iter()
                .any(|&s| self.rules[s as usize] != Rule::None),
            Blocks::Full(f) => f.iter().any(|&s| self.rules[s as usize] != Rule::None),
        }
    }

    /// State changes for the centre column (local x, y above its bottom, local z).
    pub fn edits(&self, n: &Neighbourhood, props: &PropsTable) -> Vec<([usize; 3], JavaState)> {
        let c = n.centre();
        let min_y = c.min_y();
        let mut out = Vec::new();
        for (si, section) in c.sections.iter().enumerate() {
            if !self.has_rule(section) {
                continue;
            }
            for i in 0..4096 {
                let s = section.get(i);
                if self.rules[s as usize] == Rule::None {
                    continue;
                }
                let (x, z, ly) = (i & 15, (i >> 4) & 15, (i >> 8) + si * 16);
                let y = min_y + ly as i32;
                if let Some(new) = self
                    .fix(n, props, s, x as i32, y, z as i32)
                    .filter(|&new| new != s)
                {
                    out.push(([x, ly, z], new));
                }
            }
        }
        out
    }

    fn name(&self, s: JavaState) -> &'static str {
        self.names[s as usize]
    }

    fn fix(
        &self,
        n: &Neighbourhood,
        props: &PropsTable,
        s: JavaState,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<JavaState> {
        let j = mapping::java();
        match self.rules[s as usize] {
            Rule::None => None,
            Rule::Snowy => {
                let above = self.name(n.block(x, y + 1, z));
                let snowy = matches!(
                    above,
                    "minecraft:snow" | "minecraft:snow_block" | "minecraft:powder_snow"
                );
                j.with(s, "snowy", if snowy { "true" } else { "false" })
            }
            Rule::Mushroom => {
                let mut out = s;
                let faces = [("up", 0, 1, 0), ("down", 0, -1, 0)]
                    .into_iter()
                    .chain(SIDES.map(|(f, dx, dz)| (f, dx, 0, dz)));
                for (face, dx, dy, dz) in faces {
                    if self.name(n.block(x + dx, y + dy, z + dz)) == self.name(s) {
                        out = j.with(out, face, "false")?;
                    }
                }
                Some(out)
            }
            Rule::Plant { family, up, body } => {
                let next = n.block(x, if up { y + 1 } else { y - 1 }, z);
                let continues = matches!(self.rules[next as usize], Rule::Plant { family: f, .. } if f == family);
                if continues == body {
                    return None;
                }
                let (head, body_name, _) = PLANTS[family as usize];
                let target = j
                    .find_block(if continues { body_name } else { head })?
                    .default_state();
                let props: Vec<_> = j.properties(s).collect();
                Some(
                    props
                        .iter()
                        .fold(target, |t, (k, v)| j.with(t, k, v).unwrap_or(t)),
                )
            }
            Rule::RedstoneWire => Some(self.wire(n, props, s, x, y, z)),
            Rule::Piston => {
                let facing = j.property(s, "facing")?;
                let (dx, dy, dz) = step(facing);
                let front = n.block(x + dx, y + dy, z + dz);
                let extended = self.name(front) == "minecraft:piston_head"
                    && j.property(front, "facing") == Some(facing);
                j.with(s, "extended", if extended { "true" } else { "false" })
            }
        }
    }

    /// Java's `RedStoneWireBlock.getConnectionState` from a cross (a lone wire draws as a cross).
    fn wire(
        &self,
        n: &Neighbourhood,
        props: &PropsTable,
        s: JavaState,
        x: i32,
        y: i32,
        z: i32,
    ) -> JavaState {
        let j = mapping::java();
        let conductor = |st: JavaState| props.get(st).opacity == 15;
        let up_open = !conductor(n.block(x, y + 1, z));
        let mut sides = ["none"; 4];
        for (k, &(dir, dx, dz)) in SIDES.iter().enumerate() {
            let nb = n.block(x + dx, y, z + dz);
            sides[k] = if up_open && conductor(nb) && self.is_wire(n.block(x + dx, y + 1, z + dz)) {
                "up"
            } else if self.connects(nb, dir)
                || (!conductor(nb) && self.is_wire(n.block(x + dx, y - 1, z + dz)))
            {
                "side"
            } else {
                "none"
            };
        }
        let on = sides.map(|v| v != "none");
        let (ns_empty, ew_empty) = (!on[0] && !on[1], !on[2] && !on[3]);
        for k in 0..4 {
            if !on[k] && if k < 2 { ew_empty } else { ns_empty } {
                sides[k] = "side";
            }
        }
        SIDES
            .iter()
            .zip(sides)
            .fold(s, |st, ((dir, ..), v)| j.with(st, dir, v).unwrap_or(st))
    }

    fn is_wire(&self, s: JavaState) -> bool {
        self.rules[s as usize] == Rule::RedstoneWire
    }

    /// Java `RedStoneWireBlock.shouldConnectTo`: wire, signal sources, repeaters along their axis and
    /// observers facing away.
    fn connects(&self, s: JavaState, dir: &str) -> bool {
        let name = self.name(s);
        let facing = || mapping::java().property(s, "facing").unwrap_or("");
        match name {
            "minecraft:redstone_wire" => true,
            "minecraft:repeater" => facing() == dir || facing() == opposite(dir),
            "minecraft:observer" => facing() == dir,
            _ => {
                name.ends_with("_button")
                    || name.ends_with("_pressure_plate")
                    || matches!(
                        name,
                        "minecraft:redstone_torch"
                            | "minecraft:redstone_wall_torch"
                            | "minecraft:redstone_block"
                            | "minecraft:lever"
                            | "minecraft:comparator"
                            | "minecraft:target"
                            | "minecraft:daylight_detector"
                            | "minecraft:detector_rail"
                            | "minecraft:tripwire_hook"
                            | "minecraft:trapped_chest"
                            | "minecraft:lectern"
                            | "minecraft:sculk_sensor"
                            | "minecraft:calibrated_sculk_sensor"
                            | "minecraft:lightning_rod"
                    )
            }
        }
    }
}

fn opposite(d: &str) -> &'static str {
    match d {
        "north" => "south",
        "south" => "north",
        "west" => "east",
        "east" => "west",
        "up" => "down",
        _ => "up",
    }
}

#[cfg(test)]
mod tests;

fn step(facing: &str) -> (i32, i32, i32) {
    match facing {
        "north" => (0, 0, -1),
        "south" => (0, 0, 1),
        "west" => (-1, 0, 0),
        "east" => (1, 0, 0),
        "up" => (0, 1, 0),
        _ => (0, -1, 0),
    }
}
