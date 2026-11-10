use acacia_registry::{BlockTag, prop};

use crate::aabb::Aabb;
use crate::block_factors::jump_factor;
use crate::collide::{Blocks, Colliders, Context};
use crate::fluid::FluidContact;
use crate::input::{Keys, Pose};
use crate::mth::{cos, floor, sin, DEG_TO_RAD};
use crate::support::{Support, BELOW_MOVEMENT_OFFSET};

pub const WIDTH: f32 = 0.6;
pub const STEP_HEIGHT: f32 = 0.6;
const FIT_MARGIN: f64 = 1.0E-7;

/// The held item's `use_effects` component while it is being used; vanilla's default is speed 0.2, no sprinting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemUse {
    pub speed: f32,
    pub can_sprint: bool,
}

/// Potion effects that change movement, as amplifiers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    pub speed: Option<u8>,
    pub slowness: Option<u8>,
    pub jump_boost: Option<u8>,
    pub levitation: Option<u8>,
    pub slow_falling: bool,
    pub dolphins_grace: bool,
    pub blindness: bool,
    pub weaving: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
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
    pub flying: bool,
    pub may_fly: bool,
    pub flying_speed: f32,
    pub fall_flying: bool,
    pub has_glider: bool,
    pub snow_boots: bool,
    pub has_food: bool,
    pub fast_lava: bool,
    pub effects: Effects,
    pub using_item: Option<ItemUse>,
    /// Sum of the server's flat `movement_speed` modifiers; the powder snow freeze slowdown arrives this way.
    pub speed_add: f64,
    /// Boxes of nearby entities the player can't walk through, such as boats and shulkers.
    pub solid_entities: Vec<Aabb>,
    pub in_water: bool,
    pub swimming: bool,
    pub(crate) fluids: FluidContact,
    pub(crate) was_eye_in_water: bool,
    pub(crate) pose: Pose,
    pub(crate) crouching: bool,
    pub(crate) move_vector: [f32; 2],
    pub(crate) xxa: f32,
    pub(crate) zza: f32,
    pub(crate) support: Support,
    pub(crate) fall_distance: f64,
    pub(crate) stuck: [f64; 3],
    pub(crate) in_powder_snow: bool,
    pub(crate) was_in_powder_snow: bool,
    pub(crate) moves: Vec<([f64; 3], [f64; 3], [f64; 3])>,
    pub(crate) no_jump_delay: i32,
    pub(crate) sprint_trigger: i32,
    pub jump_trigger: i32,
}

impl Player {
    pub fn at(pos: [f64; 3]) -> Player {
        Player { pos, has_food: true, flying_speed: 0.05, ..Player::default() }
    }

    pub fn fall_distance(&self) -> f64 {
        self.fall_distance
    }

    /// Ticks before the next jump on the ground.
    pub fn jump_delay(&self) -> i32 {
        self.no_jump_delay
    }

    pub fn set_jump_delay(&mut self, ticks: i32) {
        self.no_jump_delay = ticks;
    }

    pub fn crouching(&self) -> bool {
        self.crouching
    }

    pub fn pose(&self) -> Pose {
        self.pose
    }

    /// Starts crouched (or standing), as if shift had been held (or not) on the last tick.
    pub fn set_crouching(&mut self, on: bool) {
        self.crouching = on;
        self.pose = if on { Pose::Crouching } else { Pose::Standing };
    }

    pub fn bounding_box(&self) -> Aabb {
        Aabb::of_entity(self.pos, WIDTH, self.pose.height())
    }

    pub(crate) fn eye_y(&self) -> f64 {
        self.pos[1] + self.pose.eye_height() as f64
    }

    pub(crate) fn colliders<'a, W: Blocks>(&'a self, world: &'a W) -> Colliders<'a, W> {
        Colliders {
            world,
            context: Context {
                feet: self.pos[1],
                descending: self.keys.shift,
                fall_distance: self.fall_distance,
                snow_boots: self.snow_boots,
            },
            entities: &self.solid_entities,
        }
    }

    pub(crate) fn fits(&self, world: &impl Blocks, pose: Pose) -> bool {
        self.colliders(world).no_collision(&Aabb::of_entity(self.pos, WIDTH, pose.height()).deflated(FIT_MARGIN))
    }

    pub(crate) fn block_pos(&self) -> [i32; 3] {
        self.pos.map(floor)
    }

    pub(crate) fn on_pos(&self, world: &impl Blocks, offset: f32) -> [i32; 3] {
        self.support.on_pos(world, self.pos, offset)
    }

    /// The `movement_speed` attribute as a float.
    pub(crate) fn speed(&self) -> f32 {
        let mut value = 0.1_f32 as f64 + self.speed_add;
        if self.sprinting {
            value *= 1.0 + 0.3_f32 as f64;
        }
        if let Some(amp) = self.effects.speed {
            value *= 1.0 + 0.2_f32 as f64 * (amp as f64 + 1.0);
        }
        if let Some(amp) = self.effects.slowness {
            value *= 1.0 + -0.15_f32 as f64 * (amp as f64 + 1.0);
        }
        value.max(0.0) as f32
    }

    /// `Vec3.add(0, y, 0)`: adding zero turns a negative zero positive, which the bit comparison sees.
    pub(crate) fn add_y(&mut self, y: f64) {
        self.delta = [self.delta[0] + 0.0, self.delta[1] + y, self.delta[2] + 0.0];
    }

    pub(crate) fn gravity(&self) -> f64 {
        if self.delta[1] <= 0.0 && self.effects.slow_falling { 0.08_f64.min(0.01) } else { 0.08 }
    }

    pub(crate) fn in_lava(&self) -> bool {
        self.fluids.height(true) > 0.0
    }

    pub(crate) fn under_water(&self) -> bool {
        self.was_eye_in_water && self.in_water
    }

    pub(crate) fn jump_threshold(&self) -> f64 {
        if (self.pose.eye_height() as f64) < 0.4 { 0.0 } else { 0.4 }
    }

    pub(crate) fn on_climbable(&self, world: &impl Blocks) -> bool {
        let here = self.block_pos();
        let state = world.block_state(here);
        let block = state.block();
        if self.fall_flying && block.is(BlockTag::CanGlideThrough) {
            return false;
        }
        if block.is(BlockTag::Climbable) {
            return true;
        }
        if !block.is(BlockTag::Trapdoors) || state.get(prop::Open) != Some(true) {
            return false;
        }
        let below = world.block_state([here[0], here[1] - 1, here[2]]);
        below.block() == acacia_registry::Block::Ladder && below.get(prop::HorizontalFacing) == state.get(prop::HorizontalFacing)
    }

    pub(crate) fn jump_from_ground(&mut self, world: &impl Blocks) {
        let here = jump_factor(world.block_state(self.block_pos()).block());
        let below = jump_factor(world.block_state(self.on_pos(world, BELOW_MOVEMENT_OFFSET)).block());
        let factor = if here as f64 == 1.0 { below } else { here };
        let boost = self.effects.jump_boost.map_or(0.0, |amp| 0.1_f32 * (amp as f32 + 1.0));
        let power = 0.42_f32 * 1.0 * factor + boost;
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

    /// `Entity.push` from an entity overlapping the player; call before `tick`, as vanilla mobs do in their own tick.
    pub fn pushed_by(&mut self, other: [f64; 3]) {
        let (mut xa, mut za) = (other[0] - self.pos[0], other[2] - self.pos[2]);
        let mut dd = xa.abs().max(za.abs());
        if dd < 0.01_f32 as f64 {
            return;
        }
        dd = dd.sqrt();
        xa /= dd;
        za /= dd;
        let pow = (1.0 / dd).min(1.0);
        xa *= pow;
        za *= pow;
        xa *= 0.05_f32 as f64;
        za *= 0.05_f32 as f64;
        self.delta[0] += -xa;
        self.delta[2] += -za;
    }
}
