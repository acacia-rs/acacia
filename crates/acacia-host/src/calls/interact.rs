use acacia_api::{ActivateBlockParams, Face};
use acacia_bot::{ActionError, Bot};

use crate::convert;

pub(super) async fn activate_block(bot: &mut Bot, p: ActivateBlockParams) -> Result<(), ActionError> {
    bot.use_item_on_block([p.x, p.y, p.z], convert::face(p.face.unwrap_or(Face::Up))).await
}

pub(super) fn respawn(bot: &mut Bot) -> Result<(), ActionError> {
    if bot.state().player.alive {
        return Err(ActionError::NotPossible("the bot is alive".into()));
    }
    bot.respawn();
    Ok(())
}
