//! The ride in the log: where the vehicle is and how often the server corrected it, for unattended
//! checks of steering.

use acacia_bot::Bot;
use acacia_bot::proto::types::MetadataDictionaryItemKey as Key;

/// Ticks between log lines while seated.
const EVERY_TICKS: u32 = 10;

/// The yaw of a vehicle that carries the player's view round as it turns: one whose seat locks the
/// rider's rotation (a boat).
pub fn carrying_yaw(bot: &Bot) -> Option<f32> {
    bot.state().riding.seat_turn()?;
    Some(bot.state().entities.get(bot.vehicle()?.runtime_id?)?.yaw)
}

#[derive(Default)]
pub struct RideLog {
    seated_ticks: u32,
    riding: bool,
}

impl RideLog {
    /// Call once per tick.
    pub fn tick(&mut self, bot: &Bot) {
        let Some(vehicle) = bot.vehicle() else {
            if std::mem::take(&mut self.riding) {
                tracing::info!(eye = ?bot.eye_position(), "left the vehicle");
            }
            self.seated_ticks = 0;
            return;
        };
        self.riding = true;
        if self.seated_ticks.is_multiple_of(EVERY_TICKS) {
            let entity = vehicle.runtime_id.and_then(|id| bot.state().entities.get(id));
            let at = entity.map(|e| ([e.position.x, e.position.y, e.position.z], e.yaw));
            let paddles = entity.map(|e| [Key::PaddleTimeLeft, Key::PaddleTimeRight].map(|key| e.metadata.float(key)));
            let (vehicle_corrections, corrections) = bot.movement().map_or((0, 0), |m| (m.vehicle_corrections, m.corrections));
            tracing::info!(kind = vehicle.kind.as_deref(), driver = vehicle.driver, ?at, ?paddles, eye = ?bot.eye_position(), seat = ?bot.state().riding.seat_offset, seat_turn = ?bot.state().riding.seat_turn(), vehicle_corrections, corrections, "riding");
        }
        self.seated_ticks += 1;
    }
}
