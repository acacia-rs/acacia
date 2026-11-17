//! One tick of a movement trace (the bridge's MovementTrace.java): space-separated columns, floats as hex bits.

use acacia_java_physics::{Aabb, Effects, ItemUse, Keys};

pub struct Nearby {
    pub pos: [f64; 3],
    pub bounds: Aabb,
    pub pushes: bool,
    pub solid: bool,
}

pub struct Line {
    pub tick: u64,
    pub keys: Keys,
    pub on_ground: bool,
    pub sprinting: bool,
    pub yaw: f32,
    pub pitch: f32,
    pub pos: [f64; 3],
    pub delta: [f64; 3],
    pub start_pos: [f64; 3],
    pub start_delta: [f64; 3],
    pub mobility: String,
    pub flying_speed: f32,
    pub effects: Effects,
    pub nearby: Vec<Nearby>,
    pub speed_add: f64,
    pub item_use: Option<ItemUse>,
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

pub fn parse(line: &str) -> Line {
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
