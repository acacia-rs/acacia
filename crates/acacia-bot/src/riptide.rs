//! Riptide: a charged Riptide trident launches the player in water or rain (docs/research/riptide-impact-exit.md §1).

use acacia_client::proto::types::InputData;

use crate::interact::breaking::enchantment_level;
use crate::{ActionError, Bot};

const RIPTIDE: i16 = 30;
/// Ticks the trident is held: BDS launches after more than 9 server ticks; two spare against arrival jitter.
const CHARGE_TICKS: u32 = 11;

impl Bot {
    /// Charges the held Riptide trident and releases it, launching along the look. The server launches only in
    /// water or rain and not riding; the bot predicts the launch in water. Bedrock only.
    pub async fn riptide(&mut self) -> Result<(), ActionError> {
        let level = enchantment_level(self.state.held_item(), RIPTIDE);
        if level == 0 {
            return Err(ActionError::NotPossible("no Riptide trident held".into()));
        }
        if self.state.riding.is_riding() {
            return Err(ActionError::NotPossible("riding".into()));
        }
        self.use_item();
        self.queue_flag(InputData::StartUsingItem);
        self.wait_ticks(CHARGE_TICKS).await?;
        self.release_item();
        let in_rain = self.in_rain();
        if let Some(b) = self.backend.bedrock_mut()
            && let Some(movement) = &mut b.movement
            && movement.predict_riptide(level, in_rain)
            && let Some(r) = &mut b.recorder
        {
            r.write(&crate::trace::Event::Riptide(level));
        }
        Ok(())
    }

    /// BDS `Weather::isPrecipitatingAt` and the snow test at the movement's rain cell: the overworld (the only
    /// dimension with sky light), a raining biome above the snow temperature, and nothing above the cell.
    fn in_rain(&self) -> bool {
        let Some(cell @ [x, y, z]) = self.backend.movement().and_then(|m| m.rain_cell()) else { return false };
        let Some(world) = self.backend.world() else { return false };
        let (Some(view), Some(registry)) = (world.view(), world.registry()) else { return false };
        view.world().dimension().sky
            && view.biome(x, y, z).is_some_and(|b| self.state.environment.rains_in(b))
            && acacia_world::rain_reaches(view, registry, cell)
    }
}
