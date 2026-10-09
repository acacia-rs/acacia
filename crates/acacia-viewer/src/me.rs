//! The player as each tick leaves it: what the camera, the HUD and the sky read.

use acacia_bot::Bot;
use acacia_bot::proto::packets::BossEventColor;
use acacia_bot::proto::types::GameMode;
use glam::{DVec3, IVec3};

use crate::control::{Stack, stack_of};

/// The player after a tick, for the camera and the HUD.
#[derive(Debug, Clone, PartialEq)]
pub struct Me {
    /// Simulated eye position.
    pub eye: DVec3,
    /// The simulation's movement state, for view bobbing and the field of view.
    pub on_ground: bool,
    pub sprinting: bool,
    pub flying: bool,
    /// On a vehicle; `eye` is then the seat's.
    pub riding: bool,
    /// See [`crate::ride::carrying_yaw`].
    pub carrying_yaw: Option<f32>,
    /// Times this player has been hurt (the camera rolls on each).
    pub hurts: u32,
    /// Armour points worn (0 to 20), and ticks of breath left and at most while short of breath.
    pub armor: u32,
    pub air: Option<(u32, u32)>,
    pub mining: Option<(IVec3, f32)>,
    pub hotbar: u8,
    pub game_mode: GameMode,
    pub alive: bool,
    /// How hard it rains, and thunders, 0 to 1.
    pub rain: f32,
    pub thunder: f32,
    /// Lightning bolts in the world (the sky flashes while there are any): runtime id and position.
    pub bolts: Vec<(u64, DVec3)>,
    pub health: f32,
    pub max_health: f32,
    pub food: f32,
    pub xp_level: i32,
    pub xp_progress: f32,
    /// The nine hotbar stacks.
    pub items: [Option<Stack>; 9],
    pub offhand: Option<Stack>,
    /// Active effects: id, the ticks left when the server sent it (-1 for endless), the tick it
    /// was stamped with, and whether it is ambient.
    pub effects: Vec<(i32, i32, u64, bool)>,
    /// Boss bars: name, fill 0 to 1, and an index of `acacia_ui::overlay::BOSS_COLOURS`.
    pub bosses: Vec<(String, f32, usize)>,
}

pub fn me(bot: &Bot) -> Me {
    let [x, y, z] = bot.eye_position();
    let state = bot.state();
    let p = &state.player;
    let items = std::array::from_fn(|slot| stack_of(bot, state.inventory.main.get(slot)?));
    Me {
        eye: DVec3::new(x.into(), y.into(), z.into()),
        on_ground: bot.movement().is_some_and(|m| m.on_ground()),
        sprinting: bot.movement().is_some_and(|m| m.sprinting()),
        flying: bot.movement().is_some_and(|m| m.flying()),
        riding: bot.vehicle().is_some(),
        carrying_yaw: crate::ride::carrying_yaw(bot),
        hurts: state.hurts.count(p.runtime_entity_id),
        armor: state.inventory.armor.iter().filter_map(|s| state.item_name(s)).map(crate::armor::points).sum(),
        air: Some((p.air.0.max(0) as u32, p.air.1.max(1) as u32)).filter(|(air, max)| air < max),
        mining: bot.mining_progress().map(|(p, f)| (IVec3::from_array(p), f)),
        hotbar: state.inventory.selected_hotbar_slot,
        game_mode: p.game_mode,
        alive: p.alive,
        rain: state.environment.rain,
        thunder: state.environment.thunder,
        bolts: state
            .entities
            .iter()
            .filter(|e| e.kind == "minecraft:lightning_bolt")
            .map(|e| (e.runtime_id, DVec3::new(e.position.x.into(), e.position.y.into(), e.position.z.into())))
            .collect(),
        health: p.health,
        max_health: p.max_health,
        food: p.hunger,
        xp_level: p.xp_level,
        xp_progress: p.xp_progress,
        items,
        offhand: stack_of(bot, &state.inventory.offhand),
        effects: p.effects.iter().map(|e| (e.id, e.duration, e.tick, e.ambient)).collect(),
        bosses: state.environment.boss_bars.values().map(|b| (b.title.clone(), b.progress, boss_colour(b.color))).collect(),
    }
}

fn boss_colour(colour: BossEventColor) -> usize {
    use BossEventColor as C;
    match colour {
        C::Blue => 1,
        C::Red => 2,
        C::Green => 3,
        C::Yellow => 4,
        C::Purple | C::RebeccaPurple => 5,
        C::White => 6,
        C::Pink | C::Unknown(_) => 0,
    }
}
