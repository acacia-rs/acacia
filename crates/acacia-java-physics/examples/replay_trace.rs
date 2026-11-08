//! Replays a client movement trace (see the bridge's MovementTrace.java) on the default superflat world and
//! compares every tick bit for bit: `replay_trace <file> [course]`. A course has one
//! `x1 y1 z1 x2 y2 z2 block[props]` per line, later lines winning.

use acacia_java_physics::{Blocks, Keys, Player};
use acacia_registry::{Block, BlockState};

struct Fill {
    low: [i32; 3],
    high: [i32; 3],
    state: u32,
}

fn parse_state(text: &str) -> BlockState {
    let (name, props) = text.split_once('[').unwrap_or((text, ""));
    let block: Block = name.parse().expect("block name");
    let pairs: Vec<(&str, &str)> = props
        .trim_end_matches(']')
        .split(',')
        .filter_map(|pair| pair.split_once('='))
        .collect();
    BlockState::with_named(block, &pairs).expect("block properties")
}

fn parse_fill(line: &str) -> Fill {
    let cols: Vec<&str> = line.split_whitespace().collect();
    let at = |i: usize| cols[i].parse::<i32>().expect("coordinate");
    Fill {
        low: [at(0).min(at(3)), at(1).min(at(4)), at(2).min(at(5))],
        high: [at(0).max(at(3)), at(1).max(at(4)), at(2).max(at(5))],
        state: parse_state(cols[6]).id(),
    }
}

struct DefaultFlat {
    fills: Vec<Fill>,
}

impl Blocks for DefaultFlat {
    fn state(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        let pos = [x, y, z];
        if let Some(fill) = self.fills.iter().rev().find(|f| (0..3).all(|a| f.low[a] <= pos[a] && pos[a] <= f.high[a])) {
            return Some(fill.state);
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

struct Line {
    tick: u64,
    keys: Keys,
    on_ground: bool,
    sprinting: bool,
    yaw: f32,
    pitch: f32,
    pos: [f64; 3],
    delta: [f64; 3],
}

fn f64_at(cols: &[&str], i: usize) -> f64 {
    f64::from_bits(u64::from_str_radix(cols[i], 16).expect("double bits"))
}

fn f32_at(cols: &[&str], i: usize) -> f32 {
    f32::from_bits(u32::from_str_radix(cols[i], 16).expect("float bits"))
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
        pos: [f64_at(&cols, 7), f64_at(&cols, 8), f64_at(&cols, 9)],
        delta: [f64_at(&cols, 10), f64_at(&cols, 11), f64_at(&cols, 12)],
    }
}

fn seed(player: &mut Player, line: &Line) {
    player.pos = line.pos;
    player.delta = line.delta;
    player.on_ground = line.on_ground;
    player.sprinting = line.sprinting;
    player.keys = line.keys;
}

fn teleported(previous: &Line, line: &Line) -> bool {
    (0..3).any(|a| (line.pos[a] - previous.pos[a]).abs() > 2.0)
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: replay_trace <file>");
    let text = std::fs::read_to_string(path).expect("read trace");
    let lines: Vec<Line> = text.lines().map(parse).collect();
    let fills = std::env::args()
        .nth(2)
        .map(|course| std::fs::read_to_string(course).expect("read course"))
        .map_or(Vec::new(), |text| text.lines().filter(|l| !l.is_empty()).map(parse_fill).collect());
    let world = DefaultFlat { fills };
    let mut player = Player::default();
    seed(&mut player, &lines[0]);
    let (mut wrong, mut compared, mut reseeds, mut moved) = (0, 0, 0, 0.0);
    for pair in lines.windows(2) {
        let (previous, line) = (&pair[0], &pair[1]);
        if teleported(previous, line) {
            seed(&mut player, line);
            reseeds += 1;
            continue;
        }
        player.yaw = line.yaw;
        player.pitch = line.pitch;
        player.tick(&world, line.keys);
        compared += 1;
        moved += (0..3).map(|a| (line.pos[a] - previous.pos[a]).powi(2)).sum::<f64>().sqrt();
        let same = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| a[i].to_bits() == b[i].to_bits());
        if !same(player.pos, line.pos) || !same(player.delta, line.delta) || player.on_ground != line.on_ground {
            wrong += 1;
            if wrong <= 6 {
                println!("tick {} keys {:?}", line.tick, line.keys);
                println!("  pos   got {:?}\n        want {:?}", player.pos, line.pos);
                println!("  delta got {:?}\n        want {:?}", player.delta, line.delta);
                println!("  ground got {} want {} sprint got {} want {}", player.on_ground, line.on_ground, player.sprinting, line.sprinting);
            }
            seed(&mut player, line);
        }
    }
    println!("compared {compared} ticks over {moved:.2} blocks, {wrong} differ, {reseeds} teleports");
}
