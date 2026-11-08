//! Replays a client movement trace (see the bridge's MovementTrace.java) and compares every tick bit for bit.
//! A course has one `x1 y1 z1 x2 y2 z2 block[props]` per line over the default superflat world. Velocity changes
//! between ticks (knockback, pushes) are taken from the trace and counted.

use std::collections::HashMap;

use acacia_java_physics::{Aabb, Blocks, Effects, ItemUse, Keys, Player};
use acacia_registry::{Block, BlockState};

fn parse_state(text: &str) -> BlockState {
    let (name, props) = text.split_once('[').unwrap_or((text, ""));
    let block: Block = name.trim_start_matches("minecraft:").parse().expect("block name");
    let pairs: Vec<(&str, &str)> = props.trim_end_matches(']').split(',').filter_map(|p| p.split_once('=')).collect();
    BlockState::with_named(block, &pairs).expect("block properties")
}

struct Course {
    cells: HashMap<[i32; 3], u32>,
}

impl Course {
    fn load(text: &str) -> Course {
        let mut cells = HashMap::new();
        for line in text.lines().filter(|l| !l.is_empty()) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            let at = |i: usize| cols[i].parse::<i32>().expect("coordinate");
            let state = parse_state(cols[6]).id();
            for x in at(0).min(at(3))..=at(0).max(at(3)) {
                for y in at(1).min(at(4))..=at(1).max(at(4)) {
                    for z in at(2).min(at(5))..=at(2).max(at(5)) {
                        cells.insert([x, y, z], state);
                    }
                }
            }
        }
        Course { cells }
    }
}

impl Blocks for Course {
    fn state(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        if let Some(state) = self.cells.get(&[x, y, z]) {
            return Some(*state);
        }
        let block = match y {
            -64 => Block::Bedrock,
            -63 | -62 => Block::Dirt,
            -61 => Block::GrassBlock,
            _ => Block::Air,
        };
        Some(block.default_state().id())
    }

    fn min_y(&self) -> i32 {
        -64
    }
}

struct Nearby {
    pos: [f64; 3],
    bounds: Aabb,
    pushes: bool,
    solid: bool,
}

struct Line {
    tick: u64,
    keys: Keys,
    on_ground: bool,
    sprinting: bool,
    yaw: f32,
    pitch: f32,
    pos: [f64; 3],
    delta: [f64; 3],
    start_pos: [f64; 3],
    start_delta: [f64; 3],
    mobility: String,
    flying_speed: f32,
    effects: Effects,
    nearby: Vec<Nearby>,
    speed_add: f64,
    item_use: Option<ItemUse>,
}

fn f64_at(cols: &[&str], i: usize) -> f64 {
    f64::from_bits(u64::from_str_radix(cols[i], 16).expect("double bits"))
}

fn f32_at(cols: &[&str], i: usize) -> f32 {
    f32::from_bits(u32::from_str_radix(cols[i], 16).expect("float bits"))
}

fn vec_at(cols: &[&str], i: usize) -> [f64; 3] {
    [f64_at(cols, i), f64_at(cols, i + 1), f64_at(cols, i + 2)]
}

fn parse_effects(text: &str) -> Effects {
    let mut effects = Effects::default();
    for entry in text.split(',').filter(|e| *e != "-") {
        let (name, amp) = entry.split_once(':').map(|(n, a)| (n, a)).unwrap_or((entry, "0"));
        let (name, amp) = match name.strip_prefix("minecraft") {
            Some(_) => amp.split_once(':').unwrap_or((amp, "0")),
            None => (name, amp),
        };
        let amp: u8 = amp.parse().expect("amplifier");
        match name {
            "speed" => effects.speed = Some(amp),
            "slowness" => effects.slowness = Some(amp),
            "jump_boost" => effects.jump_boost = Some(amp),
            "levitation" => effects.levitation = Some(amp),
            "slow_falling" => effects.slow_falling = true,
            "dolphins_grace" => effects.dolphins_grace = true,
            "blindness" => effects.blindness = true,
            "weaving" => effects.weaving = true,
            _ => {}
        }
    }
    effects
}

fn parse_nearby(text: &str) -> Vec<Nearby> {
    text.split(';')
        .filter(|e| *e != "-")
        .map(|entry| {
            let cols: Vec<&str> = entry.split(',').collect();
            let at = |i: usize| cols[i].parse::<f64>().expect("entity value");
            Nearby {
                pos: [at(0), at(1), at(2)],
                bounds: Aabb { min: [at(3), at(4), at(5)], max: [at(6), at(7), at(8)] },
                pushes: cols[9] == "true",
                solid: cols[10] == "true",
            }
        })
        .collect()
}

fn parse(line: &str) -> Line {
    let cols: Vec<&str> = line.split(' ').collect();
    let has = |column: usize, letter: char| cols[column].contains(letter);
    Line {
        tick: cols[0].parse().expect("tick"),
        keys: Keys {
            forward: has(1, 'f'),
            backward: has(1, 'b'),
            left: has(1, 'l'),
            right: has(1, 'r'),
            jump: has(1, 'j'),
            shift: has(1, 's'),
            sprint: has(1, 'p'),
        },
        on_ground: has(2, 'g'),
        sprinting: has(2, 'p'),
        yaw: f32_at(&cols, 3),
        pitch: f32_at(&cols, 4),
        pos: vec_at(&cols, 7),
        delta: vec_at(&cols, 10),
        start_pos: vec_at(&cols, 13),
        start_delta: vec_at(&cols, 16),
        mobility: cols[19].to_string(),
        flying_speed: f32_at(&cols, 20),
        effects: parse_effects(cols[21]),
        nearby: parse_nearby(cols[22]),
        speed_add: if cols.len() > 23 { f64_at(&cols, 23) } else { 0.0 },
        item_use: cols.get(24).and_then(|c| c.split_once(':')).map(|(speed, sprint)| ItemUse {
            speed: f32::from_bits(u32::from_str_radix(speed, 16).expect("float bits")),
            can_sprint: sprint == "true",
        }),
    }
}

/// Whether a pushing entity touches the player; boats (the solid pushers here) reach out with a grown box.
fn reaches(other: &Nearby, area: &Aabb) -> bool {
    if !other.solid {
        return other.bounds.intersects(area);
    }
    let (grow, shrink) = (0.2_f32 as f64, -0.01_f32 as f64);
    let b = other.bounds;
    let grown = Aabb { min: [b.min[0] - grow, b.min[1] - shrink, b.min[2] - grow], max: [b.max[0] + grow, b.max[1] + shrink, b.max[2] + grow] };
    grown.intersects(area)
}

fn same(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| a[i].to_bits() == b[i].to_bits())
}

fn seed(player: &mut Player, line: &Line) {
    player.pos = line.pos;
    player.delta = line.delta;
    player.on_ground = line.on_ground;
    player.sprinting = line.sprinting;
    player.keys = line.keys;
}

fn apply_server_state(player: &mut Player, previous: &Line, line: &Line) {
    player.yaw = line.yaw;
    player.pitch = line.pitch;
    player.may_fly = line.mobility.contains('m');
    player.flying = previous.mobility.contains('c');
    player.has_glider = line.mobility.contains('e');
    player.snow_boots = line.mobility.contains('b');
    player.flying_speed = line.flying_speed;
    player.effects = line.effects.clone();
    player.speed_add = line.speed_add;
    player.using_item = line.item_use;
    player.solid_entities = line.nearby.iter().filter(|n| n.solid).map(|n| n.bounds).collect();
    if !line.mobility.contains('F') {
        player.fall_flying = false;
    } else if previous.mobility.contains('F') {
        player.fall_flying = true;
    }
}

#[derive(Default)]
struct Tally {
    compared: u32,
    wrong: u32,
    teleports: u32,
    pushes_matched: u32,
    external: u32,
    moved: f64,
}

/// Takes teleports and between-tick velocity changes from the trace, counting whether a push explains them.
fn apply_between_ticks(player: &mut Player, line: &Line, tally: &mut Tally) {
    if !same(player.pos, line.start_pos) {
        player.pos = line.start_pos;
        player.delta = line.start_delta;
        tally.teleports += 1;
        return;
    }
    if same(player.delta, line.start_delta) {
        return;
    }
    let mut pushed = player.clone();
    let area = player.bounding_box();
    for other in line.nearby.iter().filter(|n| n.pushes && n.bounds.intersects(&area)) {
        pushed.pushed_by(other.pos);
    }
    if same(pushed.delta, line.start_delta) {
        tally.pushes_matched += 1;
    } else {
        tally.external += 1;
    }
    player.delta = line.start_delta;
}

fn report(player: &Player, line: &Line) {
    println!("tick {} keys {:?} mobility {}", line.tick, line.keys, line.mobility);
    println!("  pos   got {:?}\n        want {:?}", player.pos, line.pos);
    println!("  delta got {:?}\n        want {:?}", player.delta, line.delta);
    println!("  ground got {} want {} sprint got {} want {}", player.on_ground, line.on_ground, player.sprinting, line.sprinting);
}

/// Returns how many ticks differ from the trace.
pub fn replay(text: &str, course: &str) -> u32 {
    let lines: Vec<Line> = text.lines().map(parse).collect();
    let world = Course::load(course);
    // The course is dumped at the first keypress; before it the world may still be changing.
    let first = lines.iter().position(|l| l.keys != Keys::default()).unwrap_or(0);
    let mut player = Player::at(lines[first].pos);
    seed(&mut player, &lines[first]);
    if lines[first].keys.jump && lines[first].mobility.contains('m') {
        player.jump_trigger = 6;
    }
    let mut tally = Tally::default();
    for pair in lines[first..].windows(2) {
        let (previous, line) = (&pair[0], &pair[1]);
        apply_server_state(&mut player, previous, line);
        apply_between_ticks(&mut player, line, &mut tally);
        player.tick(&world, line.keys);
        // The trace is written after the other entities tick, so their pushes are already in it.
        let area = player.bounding_box();
        for other in line.nearby.iter().filter(|n| n.pushes && reaches(n, &area)) {
            player.pushed_by(other.pos);
        }
        tally.compared += 1;
        tally.moved += (0..3).map(|a| (line.pos[a] - line.start_pos[a]).powi(2)).sum::<f64>().sqrt();
        let flying_ok = player.flying == line.mobility.contains('c');
        if !same(player.pos, line.pos) || !same(player.delta, line.delta) || player.on_ground != line.on_ground || !flying_ok {
            tally.wrong += 1;
            if tally.wrong <= 5 {
                report(&player, line);
            }
            seed(&mut player, line);
        }
    }
    let t = tally;
    println!(
        "compared {} ticks over {:.2} blocks, {} differ, {} teleports, {} between-tick changes ({} explained by pushes)",
        t.compared,
        t.moved,
        t.wrong,
        t.teleports,
        t.external + t.pushes_matched,
        t.pushes_matched,
    );
    t.wrong
}
