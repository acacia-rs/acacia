use crate::collide::Blocks;
use crate::mth::sqrt;
use crate::player::Player;

const SPRINT_WINDOW: i32 = 7;
const JUMP_TRIGGER: i32 = 7;
const SUFFOCATION_MARGIN: f64 = 1.0E-7;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub shift: bool,
    pub sprint: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pose {
    #[default]
    Standing,
    Crouching,
    Swimming,
    FallFlying,
}

impl Pose {
    pub fn height(self) -> f32 {
        match self {
            Pose::Standing => 1.8,
            Pose::Crouching => 1.5,
            Pose::Swimming | Pose::FallFlying => 0.6,
        }
    }

    pub fn eye_height(self) -> f32 {
        match self {
            Pose::Standing => 1.62,
            Pose::Crouching => 1.27,
            Pose::Swimming | Pose::FallFlying => 0.4,
        }
    }
}

fn impulse(positive: bool, negative: bool) -> f32 {
    match (positive, negative) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => 0.0,
    }
}

fn square_input(input: [f32; 2]) -> [f32; 2] {
    let length = sqrt(input[0] * input[0] + input[1] * input[1]);
    if length <= 0.0 {
        return input;
    }
    let scale = 1.0 / length;
    let dir = [input[0] * scale, input[1] * scale];
    let (x, y) = (dir[0].abs(), dir[1].abs());
    let tan = if y > x { x / y } else { y / x };
    let modified = (length * sqrt(1.0 + tan * tan)).min(1.0);
    [dir[0] * modified, dir[1] * modified]
}

impl Player {
    pub(crate) fn has_forward_impulse(&self) -> bool {
        self.move_vector[1] > 1.0E-5
    }

    pub(crate) fn read_keys(&mut self, keys: Keys) {
        self.keys = keys;
        let v = [impulse(keys.left, keys.right), impulse(keys.forward, keys.backward)];
        let length = sqrt(v[0] * v[0] + v[1] * v[1]);
        self.move_vector = if length < 1.0E-4 { [0.0, 0.0] } else { [v[0] / length, v[1] / length] };
    }

    pub(crate) fn moving_slowly(&self) -> bool {
        self.crouching || (self.pose == Pose::Swimming && !self.in_water)
    }

    pub(crate) fn apply_input(&mut self) {
        let mut input = self.move_vector;
        if input[0] * input[0] + input[1] * input[1] != 0.0 {
            input = input.map(|v| v * 0.98);
            if self.moving_slowly() {
                input = input.map(|v| v * 0.3_f64 as f32);
            }
            input = square_input(input);
        }
        self.xxa = input[0];
        self.zza = input[1];
    }

    fn shallow_water(&self) -> bool {
        self.in_water && !self.under_water()
    }

    fn sprint_possible(&self, allow_shallow: bool) -> bool {
        !self.effects.blindness && self.has_food && (allow_shallow || !self.shallow_water())
    }

    fn can_start_sprinting(&self) -> bool {
        !self.sprinting
            && self.has_forward_impulse()
            && self.sprint_possible(self.flying)
            && (!self.fall_flying || self.under_water())
            && (!self.moving_slowly() || self.under_water())
    }

    fn should_stop_sprinting(&self) -> bool {
        if self.swimming {
            !self.sprint_possible(true) || !self.in_water || (!self.has_forward_impulse() && !self.on_ground && !self.keys.shift)
        } else {
            !self.sprint_possible(self.flying)
                || !self.has_forward_impulse()
                || (self.horizontal_collision && !self.minor_horizontal_collision)
        }
    }

    pub(crate) fn update_sprint(&mut self, was_shift: bool, had_forward: bool) {
        if was_shift || self.keys.backward {
            self.sprint_trigger = 0;
        }
        if self.can_start_sprinting() {
            if !had_forward {
                if self.sprint_trigger > 0 {
                    self.sprinting = true;
                } else {
                    self.sprint_trigger = SPRINT_WINDOW;
                }
            }
            if self.keys.sprint {
                self.sprinting = true;
            }
        }
        if self.sprinting && self.should_stop_sprinting() {
            self.sprinting = false;
        }
    }

    /// Double-tap jump to toggle creative flight; returns whether it toggled this tick.
    pub(crate) fn update_flight_toggle(&mut self, world: &impl Blocks, was_jumping: bool) -> bool {
        if !self.may_fly || was_jumping || !self.keys.jump {
            return false;
        }
        if self.jump_trigger == 0 {
            self.jump_trigger = JUMP_TRIGGER;
            return false;
        }
        if self.swimming {
            return false;
        }
        self.flying = !self.flying;
        if self.flying && self.on_ground {
            self.jump_from_ground(world);
        }
        self.jump_trigger = 0;
        true
    }

    fn suffocates_at(&self, world: &impl Blocks, x: i32, z: i32) -> bool {
        let b = self.bounding_box();
        let (x, z) = (x as f64, z as f64);
        let area = crate::Aabb { min: [x, b.min[1], z], max: [x + 1.0, b.max[1], z + 1.0] }.deflated(SUFFOCATION_MARGIN);
        crate::inside::collides_with_suffocating(world, &area)
    }

    fn move_towards_closest_space(&mut self, world: &impl Blocks, x: f64, z: f64) {
        let (bx, bz) = (x.floor() as i32, z.floor() as i32);
        if !self.suffocates_at(world, bx, bz) {
            return;
        }
        let (xd, zd) = (x - bx as f64, z - bz as f64);
        let mut best: Option<(usize, f64)> = None;
        let mut closest = f64::MAX;
        for (axis, step, distance) in [(0, -1.0, xd), (0, 1.0, 1.0 - xd), (2, -1.0, zd), (2, 1.0, 1.0 - zd)] {
            let (nx, nz) = if axis == 0 { (bx + step as i32, bz) } else { (bx, bz + step as i32) };
            if distance < closest && !self.suffocates_at(world, nx, nz) {
                closest = distance;
                best = Some((axis, step));
            }
        }
        if let Some((axis, step)) = best {
            self.delta[axis] = 0.1 * step;
        }
    }

    pub(crate) fn push_out_of_blocks(&mut self, world: &impl Blocks) {
        let half = (crate::player::WIDTH * 0.35) as f64;
        for (sx, sz) in [(-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (1.0, 1.0)] {
            self.move_towards_closest_space(world, self.pos[0] + sx * half, self.pos[2] + sz * half);
        }
    }
}
