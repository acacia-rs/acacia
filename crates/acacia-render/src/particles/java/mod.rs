//! Java's particles (26.3 client), ported from Horizon's verified rules: the `ParticleEngine`
//! with its providers ([`make`], [`make_more`]), each class's tick ([`tick`]) and
//! `ClientLevel.animateTick` ([`animate`], [`blocks`]).
//!
//! Java keeps its generators apart, and so does [`Engine`]: the provider's (`ParticleEngine.random`,
//! [`Engine::provider`]), each particle's own (`RandomSource.create()`, seeded from
//! [`Engine::seeds`]), the one `animateTick` hands blocks ([`Engine::animate`]) and the level's,
//! which picks the cells, drips and smokes campfires ([`Engine::level`]).

mod animate;
pub mod behaviour;
mod blocks;
mod blocks_more;
pub mod classify;
mod drip;
pub mod level;
mod make;
mod make_more;
pub mod particle;
pub mod probe;
mod mth;
mod tick;
pub mod types;

use glam::{DVec3, IVec3};

use crate::particles::collide::Blocks;
pub use behaviour::{Options, Request};
pub use particle::Particle;
pub use crate::java_random::JavaRandom;
pub use types::Type;

/// `ParticleGroup.MAX_PARTICLES`; one group here.
const MAX_PARTICLES: usize = 16384;

/// A `TrackingEmitter` on an entity's box, which stays where it was hit.
struct Emitter {
    options: Options,
    feet: DVec3,
    width: f64,
    height: f64,
    life: u32,
    lifetime: u32,
    random: JavaRandom,
}

pub struct Engine {
    pub particles: Vec<Particle>,
    /// Made this tick, admitted after it (`particlesToAdd`).
    pending: Vec<Particle>,
    pub provider: JavaRandom,
    pub seeds: JavaRandom,
    pub animate: JavaRandom,
    pub level: JavaRandom,
    emitters: Vec<Emitter>,
    /// For `addParticle`'s 32-block limiter.
    pub camera: DVec3,
    /// When set, requests land here instead of spawning, as a probe's stand-in level takes them.
    pub requested: Option<Vec<Request>>,
    /// Lit campfires `animateTick` came across, smoking each tick while near.
    campfires: rustc_hash::FxHashSet<IVec3>,
    /// `Level.isRaining`, for leaves dripping.
    pub raining: bool,
    /// The biome's foliage colour (`0xRRGGBB`), which tints falling leaves.
    pub foliage: u32,
    /// The dimension drips lava from dry dripstone (`default_dripstone_particle`).
    pub nether: bool,
    /// Seeds a probe hands what it makes ([`probe::Seeds`]); the rest draw from `seeds`.
    pub given: Option<probe::Seeds>,
    /// When set, every `create` is logged here too.
    pub spawned: Option<Vec<Request>>,
}

impl Default for Engine {
    fn default() -> Self {
        Engine::seeded(0x7061_7274)
    }
}

impl Engine {
    /// Every generator derived from `seed`; the game's are seeded from the clock.
    pub fn seeded(seed: i64) -> Engine {
        let mut root = JavaRandom::new(seed);
        Engine {
            particles: Vec::new(),
            pending: Vec::new(),
            provider: JavaRandom::new(root.next_long()),
            seeds: JavaRandom::new(root.next_long()),
            animate: JavaRandom::new(root.next_long()),
            level: JavaRandom::new(root.next_long()),
            emitters: Vec::new(),
            camera: DVec3::ZERO,
            requested: None,
            campfires: Default::default(),
            raining: false,
            // FoliageColor.FOLIAGE_DEFAULT.
            foliage: 0x48_B518,
            nether: false,
            given: None,
            spawned: None,
        }
    }

    /// `ClientLevel.addParticle`: dropped beyond 32 blocks of the camera unless the type
    /// overrides the limiter.
    pub fn add(&mut self, o: &Options, p: DVec3, v: DVec3) {
        if o.ty.override_limiter() || self.requested.is_some() || p.distance_squared(self.camera) <= 1024.0 {
            self.add_always(o, p, v);
        }
    }

    /// `ClientLevel.addAlwaysVisibleParticle`.
    pub fn add_always(&mut self, o: &Options, p: DVec3, v: DVec3) {
        if let Some(requested) = &mut self.requested {
            requested.push((*o, p, v));
            return;
        }
        self.create(o, p, v);
    }

    /// `ParticleEngine.createParticle`: always made.
    pub fn create(&mut self, o: &Options, p: DVec3, v: DVec3) {
        let given = self.given.as_ref().and_then(|g| g.get(&probe::key(o.ty, p, v)).copied());
        let seed = given.unwrap_or_else(|| self.seeds.next_long());
        if let Some(log) = &mut self.spawned {
            log.push((*o, p, v));
        }
        if let Some(particle) = self.make(o, p, v, seed) {
            self.pending.push(particle);
        }
    }

    /// `ParticleEngine.createTrackingEmitter` for an entity box (crits): a particle of its own
    /// that tries 16 spots inside a quarter of the box a tick for `ticks` ticks, the first at once.
    pub fn tracking_emitter(&mut self, o: Options, feet: DVec3, width: f64, height: f64, ticks: u32) {
        let mut random = JavaRandom::new(self.seeds.next_long());
        // NoRenderParticle's constructor draws as any moving particle's does.
        Particle::base_moving(&mut random, o.ty, feet, DVec3::ZERO);
        let mut emitter = Emitter { options: o, feet, width, height, life: 0, lifetime: ticks, random };
        if self.emit(&mut emitter) {
            self.emitters.push(emitter);
        }
    }

    /// `TrackingEmitter.tick`; false once it is spent.
    fn emit(&mut self, e: &mut Emitter) -> bool {
        for _ in 0..16 {
            let r = &mut e.random;
            let (xa, ya, za) = (f64::from(r.next_float() * 2.0 - 1.0), f64::from(r.next_float() * 2.0 - 1.0), f64::from(r.next_float() * 2.0 - 1.0));
            if xa * xa + ya * ya + za * za <= 1.0 {
                let p = e.feet + DVec3::new(e.width * xa / 4.0, e.height * (0.5 + ya / 4.0), e.width * za / 4.0);
                self.add(&e.options, p, DVec3::new(xa, ya + 0.2, za));
            }
        }
        e.life += 1;
        e.life < e.lifetime
    }

    /// `ParticleProvider.createParticle` with the particle's own random seeded `seed`.
    pub fn make(&mut self, o: &Options, p: DVec3, v: DVec3, seed: i64) -> Option<Particle> {
        use Type as T;
        let mut local = JavaRandom::new(seed);
        let r = &mut local;
        let ty = o.ty;
        let mut particle = match ty {
            T::Smoke | T::LargeSmoke | T::WhiteSmoke | T::Ash | T::WhiteAsh => self.smoke(r, ty, p, v),
            T::Flame | T::SoulFireFlame | T::CopperFireFlame | T::SmallFlame => Engine::flame(r, ty, p, v),
            T::Lava => Engine::lava(r, p),
            T::DrippingWater
            | T::FallingWater
            | T::DrippingLava
            | T::FallingLava
            | T::LandingLava
            | T::DrippingHoney
            | T::FallingHoney
            | T::LandingHoney
            | T::FallingNectar
            | T::FallingSporeBlossom
            | T::DrippingObsidianTear
            | T::FallingObsidianTear
            | T::LandingObsidianTear
            | T::DrippingDripstoneWater
            | T::FallingDripstoneWater
            | T::DrippingDripstoneLava
            | T::FallingDripstoneLava => self.drip(r, ty, p),
            T::Rain | T::Splash => Engine::water_drop(r, ty, p, v),
            T::Bubble | T::BubbleColumnUp | T::SulfurBubbles => Engine::bubble(r, ty, p, v),
            T::CurrentDown => Engine::current_down(r, p),
            T::Underwater | T::CrimsonSpore | T::WarpedSpore | T::SporeBlossomAir => self.suspended(r, ty, p),
            T::Portal | T::ReversePortal => Engine::portal(r, ty, p, v),
            T::Enchant => Engine::enchant(r, p, v),
            T::Crit | T::EnchantedHit => Engine::crit(r, ty, p, v),
            T::Heart | T::AngryVillager => Engine::heart(r, ty, p),
            T::HappyVillager | T::Mycelium => self.town(r, ty, p, v),
            T::Poof => Engine::poof(r, p, v),
            T::Explosion | T::ExplosionEmitter => Engine::explosion(r, ty, p, v),
            T::CampfireCosySmoke | T::CampfireSignalSmoke => Engine::campfire(r, ty, p, v),
            T::Note => Engine::note(r, p, v),
            T::Dust | T::DustColorTransition => Engine::dust(r, o, p, v),
            T::EndRod | T::TotemOfUndying => Engine::glitter(r, ty, p, v),
            T::CherryLeaves | T::PaleOakLeaves | T::TintedLeaves => Engine::leaves(r, o, p),
            T::FallingDust => Engine::falling_dust(r, o, p),
        };
        if particle.animated {
            particle.sprite = ty.sprites().first().copied();
            particle.sprite_from_age();
        } else if particle.sprite.is_none() && !particle.no_render {
            particle.sprite = self.sprite_from_set(ty);
        }
        particle.random = local;
        particle.seed = seed;
        Some(particle)
    }

    /// `SpriteSet.get(random)`, drawn from the provider's generator.
    pub(crate) fn sprite_from_set(&mut self, ty: Type) -> Option<&'static str> {
        let sprites = ty.sprites();
        (!sprites.is_empty()).then(|| sprites[self.provider.next_int(sprites.len() as i32) as usize])
    }

    /// `ParticleEngine.tick`: emitters, live particles, then the ones made meanwhile.
    pub fn tick(&mut self, blocks: &impl Blocks) {
        for mut emitter in std::mem::take(&mut self.emitters) {
            if self.emit(&mut emitter) {
                self.emitters.push(emitter);
            }
        }
        let mut particles = std::mem::take(&mut self.particles);
        for p in &mut particles {
            self.tick_particle(p, blocks);
        }
        particles.retain(|p| !p.removed);
        self.particles = particles;
        for p in std::mem::take(&mut self.pending) {
            self.admit(p, blocks);
        }
    }

    /// `ParticleGroup.add`, after the constructor's own tick where the class has one; the last
    /// quarter fills with falling odds.
    fn admit(&mut self, mut p: Particle, blocks: &impl Blocks) {
        self.constructor_tick(&mut p, blocks);
        let size = self.particles.len();
        if size >= MAX_PARTICLES {
            return;
        }
        if size >= 12288 {
            let free = (MAX_PARTICLES - size) as f32 / 4096.0;
            if self.provider.next_float() >= free * free {
                return;
            }
        }
        self.particles.push(p);
    }

    /// `CritParticle`'s constructor ticks it once; its tick sees the level, so it runs here,
    /// before the provider's tint (`MagicProvider`).
    fn constructor_tick(&mut self, p: &mut Particle, blocks: &impl Blocks) {
        if !p.pre_tick {
            return;
        }
        p.pre_tick = false;
        self.tick_particle(p, blocks);
        if p.ty == Type::EnchantedHit {
            p.rgba[0] *= 0.3;
            p.rgba[1] *= 0.8;
        }
    }

    pub fn clear(&mut self) {
        self.particles.clear();
        self.pending.clear();
        self.emitters.clear();
        self.campfires.clear();
    }
}

impl Particle {
    /// `SingleQuadParticle.setSpriteFromAge`.
    pub fn sprite_from_age(&mut self) {
        let sprites = self.ty.sprites();
        if self.removed || sprites.is_empty() {
            return;
        }
        let last = sprites.len() as i64 - 1;
        let index = (i64::from(self.age.max(0)) * last / i64::from(self.lifetime.max(1))).clamp(0, last);
        self.sprite = Some(sprites[index as usize]);
    }
}

#[cfg(test)]
mod tests;
