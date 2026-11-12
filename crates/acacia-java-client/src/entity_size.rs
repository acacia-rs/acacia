//! Another entity's box as the vanilla client builds it: `getDimensions` (pose, baby, the type's own shapes, then the
//! scale attribute through `EntityDimensions.scale`) and `makeBoundingBox` (a shulker's peek).

use acacia_java_physics::{Aabb, sin};
use acacia_java_proto::entity_types::{ENTITY_SHAPES, EntityCollision};
use acacia_java_proto::{Error, Wire, read_varint};

use crate::entity::{Entity, HAPPY_GHAST, Mover, read_item};

/// What sizes an entity beyond its type, pose and baby flag.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// The `minecraft:scale` attribute; only living entities get it synced.
    pub scale: f32,
    /// The type's size data: a slime's or phantom's size, a pufferfish's puff state, a salmon's variant, a sniffer's
    /// state.
    pub size: i32,
    pub peek: Peek,
}

/// A shulker's lid: its attach face, the synced peek (0..100) and the amount the client has animated to.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Peek {
    pub face: i32,
    pub raw: i8,
    pub current: f32,
}

const SERIALIZER_BYTE: i32 = 0;
const SERIALIZER_INT: i32 = 1;
const SERIALIZER_DIRECTION: i32 = 12;
const SERIALIZER_SNIFFER_STATE: i32 = 35;
/// Data ids after Mob's 15, AbstractFish's bucket flag, or AgeableMob's two (slimes are ageable in 26.3).
const PHANTOM_SIZE: u8 = 16;
const CUBE_SIZE: u8 = 18;
const FISH_SIZE: u8 = 17;
const SNIFFER_STATE: u8 = 18;
const SHULKER_FACE: u8 = 16;
const SHULKER_PEEK: u8 = 17;
const SNIFFER_DIGGING: i32 = 5;
const POSE_FALL_FLYING: i32 = 1;
const POSE_SLEEPING: i32 = 2;
const POSE_SWIMMING: i32 = 3;
const POSE_SPIN_ATTACK: i32 = 4;
const POSE_CROUCHING: i32 = 5;
const POSE_LONG_JUMPING: i32 = 6;
const POSE_DYING: i32 = 7;
const POSE_SITTING: i32 = 10;
const POSE_EMERGING: i32 = 13;
const POSE_DIGGING: i32 = 14;

impl Shape {
    /// A new entity's: the type's default size data (a slime is size 1, a salmon medium).
    pub(crate) fn for_kind(name: &str) -> Shape {
        let size = match name {
            "minecraft:slime" | "minecraft:magma_cube" | "minecraft:salmon" => 1,
            _ => 0,
        };
        Shape { scale: 1.0, size, peek: Peek::default() }
    }

    /// `Shulker.updatePeekAmount`, once a client tick: the lid moves 0.05 towards the synced peek.
    pub(crate) fn tick(&mut self) {
        let p = &mut self.peek;
        let target = f32::from(p.raw) * 0.01;
        if p.current > target {
            p.current = clamp(p.current - 0.05, target, 1.0);
        } else if p.current < target {
            p.current = clamp(p.current + 0.05, 0.0, target);
        }
    }
}

/// `Mth.clamp`, which takes `min` when the bounds cross.
fn clamp(v: f32, min: f32, max: f32) -> f32 {
    if v < min { min } else { v.min(max) }
}

/// `Direction.getStepX/Y/Z` by 3D data value.
fn step(face: i32) -> [f64; 3] {
    match face {
        0 => [0.0, -1.0, 0.0],
        1 => [0.0, 1.0, 0.0],
        2 => [0.0, 0.0, -1.0],
        3 => [0.0, 0.0, 1.0],
        4 => [-1.0, 0.0, 0.0],
        _ => [1.0, 0.0, 0.0],
    }
}

impl Entity {
    /// `LivingEntity.getScale`: the attribute through the type's `sanitizeScale`.
    pub fn scale(&self) -> f32 {
        let s = self.shape.scale;
        match self.type_name() {
            "minecraft:ender_dragon" => 1.0,
            "minecraft:shulker" => s.min(3.0),
            "minecraft:happy_ghast" => s.min(1.0),
            _ => s,
        }
    }

    /// `getDimensions`: width, height and how movers collide with it.
    pub fn dimensions(&self) -> Option<(f32, f32, EntityCollision)> {
        let shape = ENTITY_SHAPES.get(usize::try_from(self.kind).ok()?)?;
        let ((width, height), fixed) = self.default_dimensions((shape.width, shape.height), shape.baby);
        let s = self.scale();
        let (width, height) = if fixed || s == 1.0 { (width, height) } else { (width * s, height * s) };
        Some((width, height, shape.collision))
    }

    /// `getDefaultDimensions` (or a fixed pose size) before the scale attribute; true when fixed.
    fn default_dimensions(&self, adult: (f32, f32), baby: Option<(f32, f32, u8)>) -> ((f32, f32), bool) {
        let avatar = self.is("minecraft:player") || self.is("minecraft:mannequin");
        match self.pose {
            POSE_SLEEPING => return ((0.2, 0.2), true),
            POSE_DYING if avatar => return ((0.2, 0.2), true),
            POSE_FALL_FLYING | POSE_SWIMMING | POSE_SPIN_ATTACK if avatar => return ((0.6, 0.6), false),
            POSE_CROUCHING if avatar => return ((0.6, 1.5), false),
            _ => {}
        }
        let (w, h) = match baby.filter(|_| self.baby) {
            Some((w, h, _)) => (w, h),
            None => adult,
        };
        let scaled = |f: f32| ((w * f, h * f), false);
        match self.type_name() {
            "minecraft:camel" | "minecraft:camel_husk" => match (self.baby, self.pose == POSE_SITTING) {
                (true, true) => ((0.95, 0.425), false),
                (true, false) => ((0.95, 1.4), false),
                (false, true) => ((adult.0, adult.1 - 1.43), false),
                (false, false) => (adult, false),
            },
            "minecraft:slime" | "minecraft:magma_cube" => ((adult.0 * self.shape.size as f32, adult.1 * self.shape.size as f32), false),
            "minecraft:pufferfish" => scaled(match self.shape.size {
                0 => 0.5,
                1 => 0.7,
                _ => 1.0,
            }),
            "minecraft:salmon" => scaled(match self.shape.size {
                0 => 0.5,
                2 => 1.5,
                _ => 1.0,
            }),
            "minecraft:phantom" => scaled(1.0 + 0.15 * self.shape.size as f32),
            "minecraft:goat" if self.pose == POSE_LONG_JUMPING => scaled(0.7),
            "minecraft:warden" if self.digging() => ((w, 1.0), true),
            "minecraft:sniffer" if self.shape.size == SNIFFER_DIGGING => {
                let age = if self.baby { 0.5 } else { 1.0 };
                ((adult.0 * age, (adult.1 - 0.4) * age), false)
            }
            _ => ((w, h), false),
        }
    }

    /// A warden digging down or emerging.
    fn digging(&self) -> bool {
        self.is("minecraft:warden") && matches!(self.pose, POSE_EMERGING | POSE_DIGGING)
    }

    /// `makeBoundingBox`: its dimensions around its feet, or a shulker's cube grown out of its attach face by the peek.
    pub fn bounding_box(&self) -> Option<Aabb> {
        if !self.is("minecraft:shulker") {
            let (width, height, _) = self.dimensions()?;
            return Some(Aabb::of_entity(self.position, width, height));
        }
        let size = f64::from(self.scale());
        let physical = 0.5 - sin(f64::from((0.5 + self.shape.peek.current) * std::f32::consts::PI)) * 0.5;
        // The opposite face's step is the attach face's negated.
        let grow = step(self.shape.peek.face).map(|v| -v * f64::from(physical) * size);
        let [x, y, z] = self.position;
        let (mut min, mut max) = ([-size * 0.5, 0.0, -size * 0.5], [size * 0.5, size, size * 0.5]);
        for a in 0..3 {
            if grow[a] < 0.0 { min[a] += grow[a] } else { max[a] += grow[a] }
        }
        Some(Aabb { min: [min[0] + x, min[1] + y, min[2] + z], max: [max[0] + x, max[1] + y, max[2] + z] })
    }

    /// Its box when it blocks `mover` (`canCollideWith`).
    pub fn collision_box(&self, mover: Mover) -> Option<Aabb> {
        let (_, _, collision) = self.dimensions()?;
        let bb = self.bounding_box()?;
        let blocks = match (collision, mover) {
            (EntityCollision::Solid, _) | (EntityCollision::Pushable, Mover::Vehicle) => true,
            (_, Mover::Player { feet }) => self.is(HAPPY_GHAST) && !self.baby && (feet >= bb.max[1] || self.stays_still),
            _ => false,
        };
        blocks.then_some(bb)
    }

    /// The type's size data from `SetEntityData` items.
    pub(crate) fn read_shape_data(&mut self, body: &[u8]) -> Result<(), Error> {
        let name = self.type_name();
        let shape = &mut self.shape;
        let int = |index| read_item(body, index, SERIALIZER_INT, read_varint);
        match name {
            "minecraft:slime" | "minecraft:magma_cube" => shape.size = int(CUBE_SIZE)?.unwrap_or(shape.size),
            "minecraft:phantom" => shape.size = int(PHANTOM_SIZE)?.unwrap_or(shape.size),
            "minecraft:pufferfish" | "minecraft:salmon" => shape.size = int(FISH_SIZE)?.unwrap_or(shape.size),
            "minecraft:sniffer" => shape.size = read_item(body, SNIFFER_STATE, SERIALIZER_SNIFFER_STATE, read_varint)?.unwrap_or(shape.size),
            "minecraft:shulker" => {
                let p = &mut shape.peek;
                p.face = read_item(body, SHULKER_FACE, SERIALIZER_DIRECTION, read_varint)?.unwrap_or(p.face);
                p.raw = read_item(body, SHULKER_PEEK, SERIALIZER_BYTE, |i| i8::read(i))?.unwrap_or(p.raw);
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "entity_size_tests.rs"]
mod tests;
