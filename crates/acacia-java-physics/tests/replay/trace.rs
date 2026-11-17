//! Replays a client movement trace (see the bridge's MovementTrace.java) and compares every tick bit for bit.
//! A course has one `x1 y1 z1 x2 y2 z2 block[props]` per line over the default superflat world. Velocity changes
//! between ticks (knockback, pushes) are taken from the trace and counted.

use std::collections::HashMap;

use acacia_java_physics::attributes::{Attribute, Operation};
use acacia_java_physics::{Aabb, Blocks, Keys, Player};
use acacia_registry::{Block, BlockState};

use crate::line::{Line, Nearby, parse};

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
    player.set_sprinting(line.sprinting);
    player.keys = line.keys;
}

/// The client's `movement_speed` modifiers for the traced effects and powder snow freeze, sprint kept as simulated.
fn sync_speed(player: &mut Player, line: &Line) {
    let effect = |id: &str, per_level: f32, amp: Option<u8>| {
        amp.map(|a| (format!("minecraft:effect.{id}"), per_level as f64 * (f64::from(a) + 1.0), Operation::AddMultipliedTotal))
    };
    let freeze = (line.speed_add != 0.0).then(|| ("minecraft:powder_snow".to_owned(), line.speed_add, Operation::AddValue));
    let modifiers: Vec<_> = [freeze, effect("speed", 0.2, line.effects.speed), effect("slowness", -0.15, line.effects.slowness)]
        .into_iter()
        .flatten()
        .collect();
    player.attributes.sync(Attribute::MovementSpeed, Attribute::MovementSpeed.default_value(), &modifiers);
    player.attributes.set_sprinting(player.sprinting);
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
    sync_speed(player, line);
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
