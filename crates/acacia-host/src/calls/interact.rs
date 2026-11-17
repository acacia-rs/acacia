use acacia_api::{ActivateBlockParams, EntityId, Face, UniqueId, Vehicle};
use acacia_bot::{ActionError, Bot};

use crate::convert;

pub(super) async fn activate_block(bot: &mut Bot, p: ActivateBlockParams) -> Result<(), ActionError> {
    bot.use_item_on_block([p.x, p.y, p.z], convert::face(p.face.unwrap_or(Face::Up))).await
}

pub(super) async fn mount(bot: &mut Bot, entity: u64) -> Result<Vehicle, ActionError> {
    let v = bot.mount(entity).await?;
    Ok(Vehicle { unique_id: UniqueId(v.unique_id), id: v.runtime_id.map(EntityId), kind: v.kind, driver: v.driver })
}

pub(super) fn respawn(bot: &mut Bot) -> Result<(), ActionError> {
    if bot.state().player.alive {
        return Err(ActionError::NotPossible("the bot is alive".into()));
    }
    bot.respawn();
    Ok(())
}
