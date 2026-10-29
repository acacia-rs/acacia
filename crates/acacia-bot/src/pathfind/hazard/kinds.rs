//! What each entity kind means to a path: its class, how fast it closes in and its fallback box. The one home of
//! the kind table (design: research/pathfind-entities-design.md §2).

/// How an entity endangers or blocks a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    /// Walks up and hits: a cost sphere.
    Melee,
    /// Blows up; once lit, the `Evade` process runs from it.
    Creeper,
    /// Shoots from afar: a sphere plus the cells it sees.
    Ranged,
    /// [`Class::Melee`] while angry, else a small sphere.
    Neutral,
    /// Harmless but in the way: waited for, then walked round.
    Passive,
    /// Collides with the player (boats; shulkers kept clear of though undrilled): its cells cannot be entered.
    /// Minecarts are walked through (`state::entities` solids).
    Solid,
    /// Flies at the player: dodged by the `Evade` process.
    Projectile,
    /// Other players and bots: kept a step away.
    Player,
}

/// One kind's entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kind {
    pub class: Class,
    /// Chase speed, blocks per tick (placeholder: vanilla movement attributes scaled to ground speed).
    pub speed: f32,
    /// (width, height) when the server sent no box.
    pub size: (f32, f32),
    /// How far the wire position sits above the box's bottom (BDS: a boat on the ground reports y + 0.375).
    pub lift: f32,
}

const fn kind(class: Class, speed: f32, w: f32, h: f32) -> Kind {
    Kind { class, speed, size: (w, h), lift: 0.0 }
}

const BOAT: Kind = Kind { lift: 0.375, ..kind(Solid, 0.0, 1.4, 0.455) };

use Class::*;

const KINDS: &[(&str, Kind)] = &[
    ("zombie", kind(Melee, 0.12, 0.6, 1.9)),
    ("husk", kind(Melee, 0.12, 0.6, 1.9)),
    ("drowned", kind(Melee, 0.12, 0.6, 1.9)),
    ("zombie_villager", kind(Melee, 0.12, 0.6, 1.9)),
    ("spider", kind(Melee, 0.15, 1.4, 0.9)),
    ("cave_spider", kind(Melee, 0.15, 0.7, 0.5)),
    ("vindicator", kind(Melee, 0.17, 0.6, 1.9)),
    ("piglin_brute", kind(Melee, 0.17, 0.6, 1.9)),
    ("silverfish", kind(Melee, 0.12, 0.4, 0.3)),
    ("slime", kind(Melee, 0.1, 2.08, 2.08)),
    ("magma_cube", kind(Melee, 0.1, 2.08, 2.08)),
    ("warden", kind(Melee, 0.15, 0.9, 2.9)),
    ("creeper", kind(Creeper, 0.1, 0.6, 1.8)),
    ("skeleton", kind(Ranged, 0.12, 0.6, 1.9)),
    ("stray", kind(Ranged, 0.12, 0.6, 1.9)),
    ("bogged", kind(Ranged, 0.12, 0.6, 1.9)),
    ("pillager", kind(Ranged, 0.17, 0.6, 1.9)),
    ("blaze", kind(Ranged, 0.12, 0.5, 1.8)),
    ("witch", kind(Ranged, 0.12, 0.6, 1.9)),
    ("enderman", kind(Neutral, 0.15, 0.6, 2.9)),
    ("piglin", kind(Neutral, 0.17, 0.6, 1.9)),
    ("zombie_pigman", kind(Neutral, 0.12, 0.6, 1.9)),
    ("wolf", kind(Neutral, 0.15, 0.6, 0.8)),
    ("bee", kind(Neutral, 0.15, 0.55, 0.5)),
    ("iron_golem", kind(Neutral, 0.12, 1.4, 2.9)),
    ("polar_bear", kind(Neutral, 0.12, 1.3, 1.4)),
    ("llama", kind(Neutral, 0.1, 0.9, 1.87)),
    ("cow", kind(Passive, 0.1, 0.9, 1.3)),
    ("mooshroom", kind(Passive, 0.1, 0.9, 1.3)),
    ("sheep", kind(Passive, 0.1, 0.9, 1.3)),
    ("pig", kind(Passive, 0.1, 0.9, 0.9)),
    ("chicken", kind(Passive, 0.1, 0.6, 0.8)),
    ("villager_v2", kind(Passive, 0.1, 0.6, 1.9)),
    ("villager", kind(Passive, 0.1, 0.6, 1.9)),
    ("wandering_trader", kind(Passive, 0.1, 0.6, 1.9)),
    ("horse", kind(Passive, 0.1, 1.4, 1.6)),
    ("donkey", kind(Passive, 0.1, 1.4, 1.6)),
    ("mule", kind(Passive, 0.1, 1.4, 1.6)),
    ("goat", kind(Passive, 0.1, 0.9, 1.3)),
    ("camel", kind(Passive, 0.1, 1.7, 2.375)),
    ("boat", BOAT),
    ("chest_boat", BOAT),
    ("shulker", kind(Solid, 0.0, 1.0, 1.0)),
    ("arrow", kind(Projectile, 0.0, 0.25, 0.25)),
    ("fireball", kind(Projectile, 0.0, 1.0, 1.0)),
    ("small_fireball", kind(Projectile, 0.0, 0.31, 0.31)),
    ("thrown_trident", kind(Projectile, 0.0, 0.25, 0.35)),
    ("shulker_bullet", kind(Projectile, 0.0, 0.625, 0.625)),
    ("player", kind(Player, 0.13, 0.6, 1.8)),
];

/// The entry for identifier `id` (`minecraft:zombie`), `None` for kinds a path ignores (items, orbs, paintings).
pub fn of(id: &str) -> Option<Kind> {
    let name = id.strip_prefix("minecraft:").unwrap_or(id);
    KINDS.iter().find(|(n, _)| *n == name).map(|(_, k)| *k)
}
