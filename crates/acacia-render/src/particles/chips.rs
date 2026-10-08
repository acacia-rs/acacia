//! The chips a broken block scatters (Java's `TerrainParticle` from `ParticleEngine.destroy`),
//! textured from the block texture array.

use glam::{DVec3, IVec3, Vec3};

use super::collide::{Blocks, sweep};
use super::rng::Rng;

/// Chips per axis of a broken block.
const GRID: i32 = 4;
const GRAVITY: f32 = 0.04;
const DRAG: f32 = 0.98;
/// Java's particle box.
const WIDTH: f64 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chip {
    pub position: DVec3,
    pub previous: DVec3,
    /// Blocks per tick.
    pub velocity: Vec3,
    /// Texture-array layer and the texel offset of its 4×4 piece.
    pub layer: u16,
    pub piece: [u8; 2],
    /// Half the quad's side, in blocks.
    pub size: f32,
    pub age: u32,
    pub lifetime: u32,
    pub on_ground: bool,
}

/// The chips of the block at `pos`, textured from `layer`.
pub fn break_block(pos: IVec3, layer: u16, rng: &mut Rng, out: &mut Vec<Chip>) {
    for (x, y, z) in (0..GRID).flat_map(|x| (0..GRID).flat_map(move |y| (0..GRID).map(move |z| (x, y, z)))) {
        let cell = (Vec3::new(x as f32, y as f32, z as f32) + 0.5) / GRID as f32;
        let mut velocity = (cell - 0.5) * 2.0;
        let spread = Vec3::new(rng.f32(), rng.f32(), rng.f32()) * 2.0 - 1.0;
        // Java: the direction from the centre plus noise, scaled to 0.15-0.25 and lifted.
        velocity = (velocity + spread * 0.4).normalize_or_zero() * (rng.f32() * 0.1 + 0.15);
        velocity.y += 0.1;
        let piece = [(rng.f32() * 3.0) as u8 * 4, (rng.f32() * 3.0) as u8 * 4];
        let lifetime = (4.0 / (rng.f32() * 0.9 + 0.1)) as u32;
        let size = 0.05 * (rng.f32() * 0.5 + 0.5) * 2.0;
        let position = pos.as_dvec3() + cell.as_dvec3();
        out.push(Chip { position, previous: position, velocity, layer, piece, size, age: 0, lifetime, on_ground: false });
    }
}

impl Chip {
    /// One game tick (Java's `Particle.tick`); false once it is gone.
    pub fn tick(&mut self, blocks: &impl Blocks) -> bool {
        self.previous = self.position;
        self.age += 1;
        if self.age >= self.lifetime {
            return false;
        }
        self.velocity.y -= GRAVITY;
        let wanted = self.velocity.as_dvec3();
        let min = self.position - DVec3::new(WIDTH / 2.0, 0.0, WIDTH / 2.0);
        let moved = sweep(blocks, min, min + DVec3::splat(WIDTH), wanted);
        self.position += moved;
        self.on_ground = wanted.y != moved.y && wanted.y < 0.0;
        if wanted.x != moved.x {
            self.velocity.x = 0.0;
        }
        if wanted.z != moved.z {
            self.velocity.z = 0.0;
        }
        self.velocity *= DRAG;
        if self.on_ground {
            self.velocity.x *= 0.7;
            self.velocity.z *= 0.7;
        }
        true
    }
}
