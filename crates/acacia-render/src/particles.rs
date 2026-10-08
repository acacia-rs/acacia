//! Particles: the chips a broken block scatters (Java's `TerrainParticle` from
//! `ParticleEngine.destroy`), simulated per game tick on the CPU and drawn as camera-facing quads.

use glam::{DVec3, IVec3, Vec3};

/// Chips per axis of a broken block.
const GRID: i32 = 4;
const GRAVITY: f32 = 0.04;
const DRAG: f32 = 0.98;
/// Particles beyond this are dropped oldest first.
const MAX: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    pub position: DVec3,
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

#[derive(Default)]
pub struct Particles {
    pub list: Vec<Particle>,
    seed: u64,
}

impl Particles {
    fn roll(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.seed >> 40) as f32 / (1u64 << 24) as f32
    }

    /// The chips of the block at `pos`, textured from `layer`.
    pub fn break_block(&mut self, pos: IVec3, layer: u16) {
        for (x, y, z) in (0..GRID).flat_map(|x| (0..GRID).flat_map(move |y| (0..GRID).map(move |z| (x, y, z)))) {
            let cell = (Vec3::new(x as f32, y as f32, z as f32) + 0.5) / GRID as f32;
            let mut velocity = (cell - 0.5) * 2.0;
            let spread = Vec3::new(self.roll(), self.roll(), self.roll()) * 2.0 - 1.0;
            // Java: the direction from the centre plus noise, scaled to 0.15-0.25 and lifted.
            velocity = (velocity + spread * 0.4).normalize_or_zero() * (self.roll() * 0.1 + 0.15);
            velocity.y += 0.1;
            let piece = [(self.roll() * 3.0) as u8 * 4, (self.roll() * 3.0) as u8 * 4];
            let lifetime = (4.0 / (self.roll() * 0.9 + 0.1)) as u32;
            let size = 0.05 * (self.roll() * 0.5 + 0.5) * 2.0;
            self.list.push(Particle { position: pos.as_dvec3() + cell.as_dvec3(), velocity, layer, piece, size, age: 0, lifetime, on_ground: false });
        }
        if self.list.len() > MAX {
            self.list.drain(..self.list.len() - MAX);
        }
    }

    /// One game tick; `solid` says whether a block cell stops a chip falling into it.
    pub fn tick(&mut self, solid: impl Fn(IVec3) -> bool) {
        self.list.retain_mut(|p| {
            p.age += 1;
            if p.age >= p.lifetime {
                return false;
            }
            p.velocity.y -= GRAVITY;
            let next = p.position + p.velocity.as_dvec3();
            if solid(next.floor().as_ivec3()) {
                // Stop on the block's top; slide to rest.
                p.velocity = Vec3::new(p.velocity.x * 0.7, 0.0, p.velocity.z * 0.7);
                p.on_ground = true;
            } else {
                p.position = next;
            }
            p.velocity *= DRAG;
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chips_fly_fall_and_expire() {
        let mut p = Particles::default();
        p.break_block(IVec3::new(0, 64, 0), 7);
        assert_eq!(p.list.len(), 64);
        assert!(p.list.iter().all(|c| c.layer == 7 && c.piece.iter().all(|&o| o <= 8) && (0.05..=0.1).contains(&c.size)));
        let floor = |c: IVec3| c.y < 64;
        for _ in 0..5 {
            p.tick(floor);
        }
        assert!(p.list.iter().all(|c| c.position.y >= 64.0), "nothing falls through the floor");
        for _ in 0..60 {
            p.tick(floor);
        }
        assert!(p.list.is_empty(), "every chip expires within 40 ticks");
    }
}
