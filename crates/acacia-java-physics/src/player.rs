use acacia_registry::BlockTag;

use crate::aabb::Aabb;
use crate::block_factors::jump_factor;
use crate::collide::{no_collision, Blocks};
use crate::mth::{cos, floor, sin, sqrt, DEG_TO_RAD};

const SPRINT_WINDOW: i32 = 7;
const WIDTH: f32 = 0.6;
const STANDING_HEIGHT: f32 = 1.8;
const CROUCHING_HEIGHT: f32 = 1.5;
pub const STEP_HEIGHT: f32 = 0.6;
const FIT_MARGIN: f64 = 1.0E-7;

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

#[derive(Clone, Debug, Default)]
pub struct Player {
    pub pos: [f64; 3],
    pub delta: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub horizontal_collision: bool,
    pub minor_horizontal_collision: bool,
    pub vertical_collision: bool,
    pub sprinting: bool,
    pub keys: Keys,
    pub(crate) crouching: bool,
    pub(crate) crouch_pose: bool,
    pub(crate) move_vector: [f32; 2],
    pub(crate) xxa: f32,
    pub(crate) zza: f32,
    pub(crate) supporting: Option<[i32; 3]>,
    pub(crate) on_ground_no_blocks: bool,
    pub(crate) fall_distance: f64,
    no_jump_delay: i32,
    sprint_trigger: i32,
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
    pub fn at(pos: [f64; 3]) -> Player {
        Player { pos, ..Player::default() }
    }

    pub fn bounding_box(&self) -> Aabb {
        let height = if self.crouch_pose { CROUCHING_HEIGHT } else { STANDING_HEIGHT };
        Aabb::of_entity(self.pos, WIDTH, height)
    }

    fn fits(&self, world: &impl Blocks, height: f32) -> bool {
        no_collision(world, &Aabb::of_entity(self.pos, WIDTH, height).deflated(FIT_MARGIN))
    }

    pub(crate) fn block_pos(&self) -> [i32; 3] {
        self.pos.map(floor)
    }

    /// `Entity.getOnPos` for offsets above 1.0E-5.
    pub(crate) fn on_pos(&self, world: &impl Blocks, offset: f32) -> [i32; 3] {
        let below = floor(self.pos[1] - offset as f64);
        let Some(support) = self.supporting else {
            return [floor(self.pos[0]), below, floor(self.pos[2])];
        };
        let block = world.block_state(support).block();
        let keeps_support = (offset as f64 <= 0.5 && block.is(BlockTag::Fences)) || block.is(BlockTag::Walls) || block.is(BlockTag::FenceGates);
        if keeps_support { support } else { [support[0], below, support[2]] }
    }

    pub(crate) fn speed(&self) -> f32 {
        let base = 0.1_f32 as f64;
        (if self.sprinting { base * (1.0 + 0.3_f32 as f64) } else { base }) as f32
    }

    fn has_forward_impulse(&self) -> bool {
        self.move_vector[1] > 1.0E-5
    }

    fn read_keys(&mut self, keys: Keys) {
        self.keys = keys;
        let v = [impulse(keys.left, keys.right), impulse(keys.forward, keys.backward)];
        let length = sqrt(v[0] * v[0] + v[1] * v[1]);
        self.move_vector = if length < 1.0E-4 { [0.0, 0.0] } else { [v[0] / length, v[1] / length] };
    }

    fn apply_input(&mut self) {
        let mut input = self.move_vector;
        if input[0] * input[0] + input[1] * input[1] != 0.0 {
            input = input.map(|v| v * 0.98);
            if self.crouching {
                input = input.map(|v| v * 0.3_f64 as f32);
            }
            input = square_input(input);
        }
        self.xxa = input[0];
        self.zza = input[1];
    }

    fn update_sprint(&mut self, was_shift: bool, had_forward: bool) {
        if was_shift || self.keys.backward {
            self.sprint_trigger = 0;
        }
        if !self.sprinting && self.has_forward_impulse() && !self.crouching {
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
        if self.sprinting && (!self.has_forward_impulse() || (self.horizontal_collision && !self.minor_horizontal_collision)) {
            self.sprinting = false;
        }
    }

    fn jump_from_ground(&mut self, world: &impl Blocks) {
        let here = jump_factor(world.block_state(self.block_pos()).block());
        let below = jump_factor(world.block_state(self.on_pos(world, 0.500001)).block());
        let factor = if here as f64 == 1.0 { below } else { here };
        let power = 0.42_f32 * 1.0 * factor;
        if power <= 1.0E-5 {
            return;
        }
        self.delta[1] = (power as f64).max(self.delta[1]);
        if self.sprinting {
            let angle = (self.yaw * DEG_TO_RAD) as f64;
            self.delta[0] += -(sin(angle) as f64) * 0.2;
            self.delta[2] += (cos(angle) as f64) * 0.2;
        }
    }

    /// One client tick: `LocalPlayer.aiStep` down to `travel`, then the pose update.
    // TODO: fluids, climbing, flight, gliding, effects, cobweb, moveTowardsClosestSpace.
    pub fn tick(&mut self, world: &impl Blocks, keys: Keys) {
        if self.sprint_trigger > 0 {
            self.sprint_trigger -= 1;
        }
        let was_shift = self.keys.shift;
        let had_forward = self.has_forward_impulse();
        self.crouching = self.fits(world, CROUCHING_HEIGHT) && (was_shift || !self.fits(world, STANDING_HEIGHT));
        self.read_keys(keys);
        self.update_sprint(was_shift, had_forward);

        if self.no_jump_delay > 0 {
            self.no_jump_delay -= 1;
        }
        if self.delta[0] * self.delta[0] + self.delta[2] * self.delta[2] < 9.0E-6 {
            self.delta[0] = 0.0;
            self.delta[2] = 0.0;
        }
        if self.delta[1].abs() < 0.003 {
            self.delta[1] = 0.0;
        }
        self.apply_input();
        if !keys.jump {
            self.no_jump_delay = 0;
        } else if self.on_ground && self.no_jump_delay == 0 {
            self.jump_from_ground(world);
            self.no_jump_delay = 10;
        }
        self.travel_in_air(world);
        self.update_pose(world);
    }

    fn update_pose(&mut self, world: &impl Blocks) {
        let standing_fits = self.fits(world, STANDING_HEIGHT);
        let crouch_fits = self.fits(world, CROUCHING_HEIGHT);
        self.crouch_pose = (self.keys.shift || !standing_fits) && crouch_fits;
    }
}
