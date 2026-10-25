//! `Block.animateTick` for torches, furnaces, campfires, leaves, portals, candles and the other
//! blocks that only need their own state and a neighbour or two. `self.animate` is the random
//! `ClientLevel.animateTick` hands each block; sounds still take their draws. Positions add up
//! in Java's order (`pos.getX() + 0.5 + d` is `(x + 0.5) + d`), as sums round by grouping.

use glam::{DVec3, IVec3};

use super::Engine;
use super::behaviour::Options;
use super::classify::{Animate, Tint};
use super::level;
use super::types::Type;
use crate::particles::collide::Blocks;

/// `Direction.values()` order, which `Direction.getRandom` indexes.
pub const DIRECTIONS: [IVec3; 6] = [IVec3::NEG_Y, IVec3::Y, IVec3::NEG_Z, IVec3::Z, IVec3::NEG_X, IVec3::X];

/// `CandleBlock.PARTICLE_OFFSETS` in sixteenths, by candle count.
const CANDLE_WICKS: [&[[f64; 3]]; 4] = [
    &[[8.0, 8.0, 8.0]],
    &[[6.0, 7.0, 8.0], [10.0, 8.0, 7.0]],
    &[[8.0, 5.0, 10.0], [6.0, 7.0, 8.0], [9.0, 8.0, 7.0]],
    &[[7.0, 5.0, 9.0], [10.0, 7.0, 9.0], [6.0, 7.0, 6.0], [9.0, 8.0, 6.0]],
];

pub fn redstone() -> Options {
    Options::dust([1.0, 0.0, 0.0], 1.0)
}

fn f(v: f32) -> f64 {
    f64::from(v)
}

impl Engine {
    /// A sound's volume and pitch draws, `floats` of them.
    pub(super) fn sound(&mut self, floats: u32) {
        for _ in 0..floats {
            self.animate.float();
        }
    }

    pub(super) fn block_animate(&mut self, blocks: &impl Blocks, kind: Animate, cell: IVec3) {
        use Animate as A;
        let c = cell.as_dvec3();
        let zero = DVec3::ZERO;
        match kind {
            A::Torch(flame, wall) => {
                let mut p = c + DVec3::new(0.5, 0.7, 0.5);
                if let Some(s) = wall {
                    p += DVec3::new(0.27 * f64::from(s.x), 0.22, 0.27 * f64::from(s.z));
                }
                self.simple(Type::Smoke, p, zero);
                self.simple(flame, p, zero);
            }
            A::RedstoneTorch(wall) => {
                let r = &mut self.animate;
                let mut p = c + DVec3::new(0.5, 0.7, 0.5);
                p += DVec3::new((r.double() - 0.5) * 0.2, (r.double() - 0.5) * 0.2, (r.double() - 0.5) * 0.2);
                if let Some(s) = wall {
                    p += DVec3::new(0.27 * f64::from(s.x), 0.22, 0.27 * f64::from(s.z));
                }
                self.add(&redstone(), p, zero);
            }
            A::Furnace { facing, blast } => {
                let r = &mut self.animate;
                let _crackle = r.double() < 0.1;
                let ss = r.double() * 0.6 - 0.3;
                let dx = if facing.x != 0 { f64::from(facing.x) * 0.52 } else { ss };
                let dy = r.double() * if blast { 9.0 } else { 6.0 } / 16.0;
                let dz = if facing.z != 0 { f64::from(facing.z) * 0.52 } else { ss };
                let p = c + DVec3::new(0.5, 0.0, 0.5) + DVec3::new(dx, dy, dz);
                self.simple(Type::Smoke, p, zero);
                if !blast {
                    self.simple(Type::Flame, p, zero);
                }
            }
            A::Smoker => {
                let _sound = self.animate.double() < 0.1;
                self.simple(Type::Smoke, c + DVec3::new(0.5, 0.0, 0.5) + DVec3::new(0.0, 1.1, 0.0), zero);
            }
            A::Campfire { lava } => {
                if self.animate.below(10) == 0 {
                    self.sound(2);
                }
                if lava && self.animate.below(5) == 0 {
                    let mut i = 0;
                    while i < self.animate.below(1) + 1 {
                        let r = &mut self.animate;
                        let v = DVec3::new(f(r.float() / 2.0), 5.0e-5, f(r.float() / 2.0));
                        self.simple(Type::Lava, c + DVec3::splat(0.5), v);
                        i += 1;
                    }
                }
            }
            A::Leaves(falling) => self.leaves_animate(blocks, cell, falling),
            A::NetherPortal => {
                if self.animate.below(100) == 0 {
                    self.sound(1);
                }
                let same = |s: IVec3| blocks.block(cell + s).is_some_and(|b| b.name == "minecraft:portal");
                let along_z = same(IVec3::NEG_X) || same(IVec3::X);
                for _ in 0..4 {
                    let r = &mut self.animate;
                    let mut p = c + DVec3::new(r.double(), r.double(), r.double());
                    let mut v = DVec3::new(f(r.float() - 0.5) * 0.5, f(r.float() - 0.5) * 0.5, f(r.float() - 0.5) * 0.5);
                    let flip = f64::from(r.below(2) * 2 - 1);
                    if along_z {
                        p.z = c.z + 0.5 + 0.25 * flip;
                        v.z = f(r.float() * 2.0) * flip;
                    } else {
                        p.x = c.x + 0.5 + 0.25 * flip;
                        v.x = f(r.float() * 2.0) * flip;
                    }
                    self.simple(Type::Portal, p, v);
                }
            }
            A::EnchantingTable => {
                for ox in -2..=2 {
                    for oy in 0..=1 {
                        for oz in -2..=2 {
                            if ox * ox != 4 && oz * oz != 4 {
                                continue;
                            }
                            let offset = IVec3::new(ox, oy, oz);
                            if self.animate.below(16) != 0 || !level::bookshelf_reaches(blocks, cell, offset) {
                                continue;
                            }
                            let r = &mut self.animate;
                            let v = DVec3::new(f(ox as f32 + r.float() - 0.5), f(oy as f32 - r.float() - 1.0), f(oz as f32 + r.float() - 0.5));
                            self.simple(Type::Enchant, c + DVec3::new(0.5, 2.0, 0.5), v);
                        }
                    }
                }
            }
            A::EndRod(facing) => {
                let r = &mut self.animate;
                let p = c + DVec3::splat(0.55) - DVec3::new(f(r.float() * 0.1), f(r.float() * 0.1), f(r.float() * 0.1));
                let d = f(0.4 - (r.float() + r.float()) * 0.4);
                if r.below(5) == 0 {
                    let v = DVec3::new(r.gaussian() * 0.005, r.gaussian() * 0.005, r.gaussian() * 0.005);
                    self.simple(Type::EndRod, p + facing.as_dvec3() * d, v);
                }
            }
            A::SporeBlossom => {
                let r = &mut self.animate;
                let p = c + DVec3::new(r.double(), 0.7, r.double());
                self.simple(Type::FallingSporeBlossom, p, zero);
                for _ in 0..14 {
                    let r = &mut self.animate;
                    let x = r.below(21) - 10;
                    let y = -r.below(10);
                    let z = r.below(21) - 10;
                    let at = cell + IVec3::new(x, y, z);
                    if !level::full_collision(blocks, at) {
                        let r = &mut self.animate;
                        let p = at.as_dvec3() + DVec3::new(r.double(), r.double(), r.double());
                        self.simple(Type::SporeBlossomAir, p, zero);
                    }
                }
            }
            A::CryingObsidian => {
                if self.animate.below(5) == 0 {
                    let d = DIRECTIONS[self.animate.below(6) as usize];
                    if d != IVec3::Y && !level::sturdy(blocks, cell + d) {
                        let r = &mut self.animate;
                        let mut off = |s: i32| if s == 0 { r.double() } else { 0.5 + f64::from(s) * 0.6 };
                        let p = c + DVec3::new(off(d.x), off(d.y), off(d.z));
                        self.simple(Type::DrippingObsidianTear, p, zero);
                    }
                }
            }
            A::WetSponge => {
                let d = DIRECTIONS[self.animate.below(6) as usize];
                if d != IVec3::Y && !level::sturdy(blocks, cell + d) {
                    let r = &mut self.animate;
                    let mut p = c;
                    if d == IVec3::NEG_Y {
                        p += DVec3::new(r.double(), -0.05, r.double());
                    } else {
                        p.y += r.double() * 0.8;
                        let (along, across) = (r.double(), if d.x > 0 || d.z > 0 { 1.1 } else { 0.05 });
                        p += if d.x != 0 { DVec3::new(across, 0.0, along) } else { DVec3::new(along, 0.0, across) };
                    }
                    self.simple(Type::DrippingWater, p, zero);
                }
            }
            A::Mycelium => {
                if self.animate.below(10) == 0 {
                    let r = &mut self.animate;
                    let p = c + DVec3::new(r.double(), 1.1, r.double());
                    self.simple(Type::Mycelium, p, zero);
                }
            }
            A::Candles(count) => {
                for &[x, y, z] in CANDLE_WICKS[usize::from(count.clamp(1, 4) - 1)] {
                    self.candle(c + DVec3::new(x, y, z) / 16.0);
                }
            }
            A::CandleCake => self.candle(c + DVec3::new(0.5, 1.0, 0.5)),
            A::BrewingStand => {
                let r = &mut self.animate;
                let p = c + DVec3::new(0.4, 0.7, 0.4) + DVec3::new(f(r.float() * 0.2), f(r.float() * 0.3), f(r.float() * 0.2));
                self.simple(Type::Smoke, p, zero);
            }
            A::EnderChest => {
                for _ in 0..3 {
                    let r = &mut self.animate;
                    let fx = r.below(2) * 2 - 1;
                    let fz = r.below(2) * 2 - 1;
                    let p = DVec3::new(c.x + 0.5 + 0.25 * f64::from(fx), c.y + f(r.float()), c.z + 0.5 + 0.25 * f64::from(fz));
                    let v = DVec3::new(f(r.float()) * f64::from(fx), f(r.float() - 0.5) * 0.125, f(r.float()) * f64::from(fz));
                    self.simple(Type::Portal, p, v);
                }
            }
            A::EndPortal => {
                let r = &mut self.animate;
                let p = c + DVec3::new(r.double(), 0.8, r.double());
                self.simple(Type::Smoke, p, zero);
            }
            _ => self.block_animate_more(blocks, kind, cell),
        }
    }

    /// `LeavesBlock` and `FallingParticlesLeavesBlock.animateTick`.
    fn leaves_animate(&mut self, blocks: &impl Blocks, cell: IVec3, falling: Option<(Type, f32, Tint)>) {
        let below = cell - IVec3::Y;
        if self.raining && level::sky_above(blocks, cell + IVec3::Y) && self.animate.below(15) == 1 && !level::sturdy(blocks, below) {
            self.below(cell, Options::simple(Type::DrippingWater));
        }
        let Some((ty, chance, tint)) = falling else { return };
        if self.animate.float() < chance && !level::sturdy(blocks, below) {
            let options = match tint {
                Tint::Foliage => Options::colored(ty, self.foliage),
                Tint::Fixed(rgb) => Options::colored(ty, rgb),
                Tint::Untinted => Options::simple(ty),
            };
            self.below(cell, options);
        }
    }

    /// `ParticleUtils.spawnParticleBelow`.
    pub(super) fn below(&mut self, cell: IVec3, o: Options) {
        let r = &mut self.animate;
        let p = cell.as_dvec3() + DVec3::new(r.double(), -0.05, r.double());
        self.add(&o, p, DVec3::ZERO);
    }

    /// `AbstractCandleBlock.addParticlesAndSound`.
    fn candle(&mut self, wick: DVec3) {
        let chance = self.animate.float();
        if chance < 0.3 {
            self.simple(Type::Smoke, wick, DVec3::ZERO);
            if chance < 0.17 {
                self.sound(2);
            }
        }
        self.simple(Type::SmallFlame, wick, DVec3::ZERO);
    }
}
