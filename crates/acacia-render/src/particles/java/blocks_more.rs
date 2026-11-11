//! `Block.animateTick` for falling blocks, redstone, fire, bubble columns, sculk, sulfur,
//! dripstone and the blocks that only roll for a sound.

use glam::{DVec3, IVec3};

use super::Engine;
use super::behaviour::Options;
use super::blocks::{DIRECTIONS, redstone};
use super::classify::Animate;
use super::level::{self, Fluid};
use super::types::Type;
use crate::particles::collide::Blocks;

fn f(v: f32) -> f64 {
    f64::from(v)
}

/// `RedstoneWireBlock.COLORS`, as the packed int `DustParticleOptions` keeps.
pub fn wire_color(power: u8) -> [f32; 3] {
    let p = f32::from(power) / 15.0;
    let red = p * 0.6 + if p > 0.0 { 0.4 } else { 0.3 };
    [red, (p * p * 0.7 - 0.5).clamp(0.0, 1.0), (p * p * 0.6 - 0.7).clamp(0.0, 1.0)]
}

/// `BlockBehaviour.OffsetType.XZ` for a block whose offset reaches `max`.
pub fn offset_xz(cell: IVec3, max: f32) -> DVec3 {
    let seed = level::seed(cell.x, 0, cell.z);
    let axis = |bits: i64| (((bits & 15) as f32 / 15.0) as f64 - 0.5) * 0.5;
    DVec3::new(axis(seed).clamp(-f(max), f(max)), 0.0, axis(seed >> 8).clamp(-f(max), f(max)))
}

impl Engine {
    pub(super) fn block_animate_more(&mut self, blocks: &impl Blocks, kind: Animate, cell: IVec3) {
        use Animate as A;
        let c = cell.as_dvec3();
        let zero = DVec3::ZERO;
        match kind {
            A::Falling { color, sand } => {
                if self.animate.next_int(16) == 0 && level::free(blocks, cell - IVec3::Y) {
                    self.below(cell,Options::colored(Type::FallingDust, color));
                }
                if sand && blocks.block(cell + IVec3::Y).is_some_and(|b| b.name == "minecraft:air") {
                    self.animate.next_int(2100);
                }
            }
            A::Brushable(color) => {
                if self.animate.next_int(16) == 0 && level::free(blocks, cell - IVec3::Y) {
                    self.below(cell,Options::colored(Type::FallingDust, color));
                }
            }
            A::RedstoneOre => {
                for d in DIRECTIONS {
                    if level::solid_render(blocks, cell + d) {
                        continue;
                    }
                    // RedStoneOreBlock draws from the level's random, not the one it is handed.
                    let r = &mut self.level;
                    let mut at = |s: i32| if s != 0 { 0.5 + 0.5625 * f64::from(s) } else { f(r.next_float()) };
                    let p = c + DVec3::new(at(d.x), at(d.y), at(d.z));
                    self.add(&redstone(), p, zero);
                }
            }
            A::RedstoneWire(power) => {
                let color = Options::dust(wire_color(power), 1.0);
                let sides = level::wire_sides(blocks, cell);
                for (i, h) in [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X].into_iter().enumerate() {
                    match sides[i] {
                        level::Side::Up => {
                            self.wire_line(&color, c, h, IVec3::Y, -0.5, 0.5);
                            self.wire_line(&color, c, IVec3::NEG_Y, h, 0.0, 0.5);
                        }
                        level::Side::Side => self.wire_line(&color, c, IVec3::NEG_Y, h, 0.0, 0.5),
                        level::Side::None => self.wire_line(&color, c, IVec3::NEG_Y, h, 0.0, 0.3),
                    }
                }
            }
            A::Repeater { facing, delay } => {
                let r = &mut self.animate;
                let p = c + DVec3::new(0.5, 0.4, 0.5) + DVec3::new((r.next_double() - 0.5) * 0.2, (r.next_double() - 0.5) * 0.2, (r.next_double() - 0.5) * 0.2);
                let offset = (if r.next_boolean() { f32::from(delay) * 2.0 - 1.0 } else { -5.0 }) / 16.0;
                let shift = DVec3::new(f(offset) * f64::from(facing.x), 0.0, f(offset) * f64::from(facing.z));
                self.add(&redstone(), p + shift, zero);
            }
            A::Lever { facing, attached } => {
                if self.animate.next_float() < 0.25 {
                    let p = c + DVec3::splat(0.5) + 0.1 * facing.as_dvec3() + 0.2 * attached.as_dvec3();
                    self.add(&Options::dust([1.0, 0.0, 0.0], 0.5), p, zero);
                }
            }
            A::RespawnAnchor => {
                self.animate.next_int(100);
                let r = &mut self.animate;
                let p = c + DVec3::new(0.5, 1.0, 0.5) + DVec3::new(0.5 - r.next_double(), 0.0, 0.5 - r.next_double());
                let ya = f(r.next_float()) * 0.04;
                self.simple(Type::ReversePortal, p, DVec3::new(0.0, ya, 0.0));
            }
            A::BubbleColumn { drag } => {
                if drag {
                    self.add_always(&Options::simple(Type::CurrentDown), c + DVec3::new(0.5, 0.8, 0.0), zero);
                } else {
                    let up = DVec3::new(0.0, 0.04, 0.0);
                    self.add_always(&Options::simple(Type::BubbleColumnUp), c + DVec3::new(0.5, 0.0, 0.5), up);
                    let r = &mut self.animate;
                    let p = c + DVec3::new(f(r.next_float()), f(r.next_float()), f(r.next_float()));
                    self.add_always(&Options::simple(Type::BubbleColumnUp), p, up);
                }
                if self.animate.next_int(200) == 0 {
                    self.sound(2);
                }
            }
            A::Fire { soul } => self.fire(blocks, cell, soul),
            A::SculkSensor => {
                let d = DIRECTIONS[self.animate.next_int(6) as usize];
                if d.y == 0 {
                    let r = &mut self.animate;
                    let x = if d.x == 0 { 0.5 - r.next_double() } else { f64::from(d.x) * 0.6 };
                    let z = if d.z == 0 { 0.5 - r.next_double() } else { f64::from(d.z) * 0.6 };
                    let ya = f(r.next_float()) * 0.04;
                    let sculk = Options::transition([0x39 as f32 / 255.0, 0xD6 as f32 / 255.0, 0xE0 as f32 / 255.0], [1.0, 0.0, 0.0], 1.0);
                    self.add(&sculk, c + DVec3::new(0.5, 0.25, 0.5) + DVec3::new(x, 0.0, z), DVec3::new(0.0, ya, 0.0));
                }
            }
            A::PotentSulfur => {
                if level::fluid(blocks, cell + IVec3::Y).is_source() && level::fluid(blocks, cell + IVec3::Y).fluid == Fluid::Water {
                    for _ in 0..2 {
                        let r = &mut self.animate;
                        let p = c + DVec3::Y + DVec3::new(f(r.next_float()), f(r.next_float()), f(r.next_float()));
                        self.add_always(&Options::simple(Type::SulfurBubbles), p, zero);
                    }
                    self.animate.next_int(10);
                }
            }
            A::WitherRose => {
                // The flower's shape (5 to 11 sixteenths), moved by its offset: its bounds' centre.
                let o = offset_xz(cell, 0.25);
                let mid = |o: f64| (0.3125 + o) + 0.5 * ((0.6875 + o) - (0.3125 + o));
                let centre = c + DVec3::new(mid(o.x), 0.0, mid(o.z));
                for _ in 0..3 {
                    if self.animate.next_boolean() {
                        let r = &mut self.animate;
                        let p = DVec3::new(centre.x + r.next_double() / 5.0, c.y + (0.5 - r.next_double()), centre.z + r.next_double() / 5.0);
                        self.simple(Type::Smoke, p, zero);
                    }
                }
            }
            A::DriedGhast { .. } => {
                let centre = c + DVec3::splat(0.5);
                self.animate.next_int(40);
                if self.animate.next_int(6) == 0 {
                    if level::fluid(blocks, cell).fluid == Fluid::Water {
                        let r = &mut self.animate;
                        let p = centre + DVec3::new(f((r.next_float() * 2.0 - 1.0) / 3.0), 0.4, f((r.next_float() * 2.0 - 1.0) / 3.0));
                        let v = DVec3::new(0.0, f(r.next_float()), 0.0);
                        self.simple(Type::HappyVillager, p, v);
                    } else {
                        self.simple(Type::WhiteSmoke, centre, DVec3::new(0.0, 0.02, 0.0));
                    }
                }
            }
            A::Beehive => {
                let mut i = 0;
                while i < self.animate.next_int(1) + 1 {
                    self.beehive_drip(blocks, cell);
                    i += 1;
                }
            }
            A::Stalactite => self.stalactite(blocks, cell),
            A::Sound(bound) => {
                if self.animate.next_int(bound) == 0 {
                    self.sound(0);
                }
            }
            A::DeadBush => {
                let below = blocks.block(cell - IVec3::Y).map_or("", |b| b.name);
                if self.animate.next_int(130) == 0 && (below == "minecraft:red_sand" || below.ends_with("terracotta")) {
                    self.animate.next_int(3);
                }
            }
            _ => {}
        }
    }

    /// `RedstoneWireBlock.spawnParticlesAlongLine`.
    fn wire_line(&mut self, color: &Options, c: DVec3, side: IVec3, along: IVec3, from: f32, to: f32) {
        let span = to - from;
        let r = &mut self.animate;
        if r.next_float() < 0.2 * span {
            let on = f(from + span * r.next_float());
            let p = DVec3::splat(0.5) + side.as_dvec3() * f(0.4375) + along.as_dvec3() * on;
            self.add(color, c + p, DVec3::ZERO);
        }
    }

    /// `BaseFireBlock.animateTick`: smoke on its floor, or along the faces it burns.
    fn fire(&mut self, blocks: &impl Blocks, cell: IVec3, soul: bool) {
        if self.animate.next_int(24) == 0 {
            self.sound(2);
        }
        let burns = |at: IVec3| soul || level::flammable(blocks, at);
        let c = cell.as_dvec3();
        let smoke = |engine: &mut Engine, p: DVec3| engine.simple(Type::LargeSmoke, p, DVec3::ZERO);
        if !burns(cell - IVec3::Y) && !level::sturdy(blocks, cell - IVec3::Y) {
            // West, east, north, south, then above; each face's near edge is 0.1 deep.
            for (side, edge) in [(IVec3::NEG_X, 0), (IVec3::X, 0), (IVec3::NEG_Z, 2), (IVec3::Z, 2), (IVec3::Y, 1)] {
                if !burns(cell + side) {
                    continue;
                }
                for _ in 0..2 {
                    let r = &mut self.animate;
                    let mut p = [r.next_double(), r.next_double(), r.next_double()];
                    let near = r_edge(p[edge]);
                    let base = c.to_array();
                    p = std::array::from_fn(|i| match i == edge {
                        true if side.x + side.y + side.z > 0 => base[i] + 1.0 - near,
                        true => base[i] + near,
                        false => base[i] + p[i],
                    });
                    smoke(self, DVec3::from(p));
                }
            }
        } else {
            for _ in 0..3 {
                let r = &mut self.animate;
                let p = DVec3::new(c.x + r.next_double(), c.y + r.next_double() * 0.5 + 0.5, c.z + r.next_double());
                smoke(self, p);
            }
        }
    }

    /// `BeehiveBlock.trySpawnDripParticles`, from the level's random.
    fn beehive_drip(&mut self, blocks: &impl Blocks, cell: IVec3) {
        if level::fluid(blocks, cell).fluid != Fluid::None || self.level.next_float() < 0.3 {
            return;
        }
        let Some(shape) = level::extent(blocks, cell) else { return };
        if shape.top < 1.0 || level::impermeable(blocks, cell) {
            return;
        }
        let y = f64::from(cell.y);
        let height = if shape.bottom > 0.0 {
            y + shape.bottom - 0.05
        } else {
            let below = cell - IVec3::Y;
            let open = level::extent(blocks, below).is_some_and(|b| b.top < 1.0) || !level::full_collision(blocks, below);
            if !open || level::fluid(blocks, below).fluid != Fluid::None {
                return;
            }
            y - 0.05
        };
        let r = &mut self.level;
        let (a, b) = (r.next_double(), r.next_double());
        let c = cell.as_dvec3();
        let p = DVec3::new(level::lerp(a, c.x + shape.x[0], c.x + shape.x[1]), height, level::lerp(b, c.z + shape.z[0], c.z + shape.z[1]));
        self.simple(Type::DrippingHoney, p, DVec3::ZERO);
    }

    /// `PointedDripstoneBlock.animateTick` for a free-hanging tip.
    fn stalactite(&mut self, blocks: &impl Blocks, cell: IVec3) {
        if level::fluid(blocks, cell).fluid != Fluid::None {
            return;
        }
        let chance = self.animate.next_float();
        if chance > 0.12 {
            return;
        }
        let Some(above) = level::above_stalactite(blocks, cell) else { return };
        let fluid = if blocks.block(above).is_some_and(|b| b.name == "minecraft:mud") && !self.nether {
            Fluid::Water
        } else {
            level::fluid(blocks, above).fluid
        };
        if chance >= 0.02 && fluid == Fluid::None {
            return;
        }
        let ty = match fluid {
            Fluid::Lava => Type::DrippingDripstoneLava,
            Fluid::Water => Type::DrippingDripstoneWater,
            Fluid::None if self.nether => Type::DrippingDripstoneLava,
            Fluid::None => Type::DrippingDripstoneWater,
        };
        // The tip's shape starts 5 pixels up; the drop hangs a pixel under that.
        let offset = offset_xz(cell, 0.125);
        let p = cell.as_dvec3() + DVec3::new(0.5, 5.0 / 16.0, 0.5) + DVec3::new(offset.x, -0.0625, offset.z);
        self.simple(ty, p, DVec3::ZERO);
    }
}

/// A fire face's depth: `nextDouble() * 0.1F`, the float widened.
fn r_edge(d: f64) -> f64 {
    d * f(0.1)
}
