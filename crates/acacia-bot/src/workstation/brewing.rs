//! Brewing stand: plain container slots, Bedrock's ingredient 0, bottles 1-3, blaze powder 4, Java's bottles 0-2,
//! ingredient 3, blaze powder 4 (`BrewingStandMenu`).

use std::time::Duration;

use acacia_client::proto::types::WindowType;
use acacia_client::proto::RawPacket;
use acacia_physics::BlockPos;

use crate::items::SlotRef;
use crate::{ActionError, Bot};

#[derive(Clone, Copy)]
struct Layout {
    ingredient: u8,
    bottles: [u8; 3],
    fuel: u8,
}

const BEDROCK: Layout = Layout { ingredient: 0, bottles: [1, 2, 3], fuel: 4 };
const JAVA: Layout = Layout { ingredient: 3, bottles: [0, 1, 2], fuel: 4 };
/// Vanilla brewing time (400 ticks) plus slack.
const BREW_TIMEOUT: Duration = Duration::from_secs(25);

impl Bot {
    /// Brews at the stand at `pos`: puts the (up to three) `bottles` in, one blaze powder if the
    /// fuel slot is empty, and one `ingredient`; waits for the brew and takes the bottles back out.
    pub async fn brew(&mut self, pos: BlockPos, bottles: &[SlotRef], ingredient: &str) -> Result<(), ActionError> {
        if bottles.is_empty() || bottles.len() > BEDROCK.bottles.len() {
            return Err(ActionError::NotPossible("a brewing stand takes one to three bottles".into()));
        }
        self.open_station_and_look(pos, &[WindowType::BrewingStand]).await?;
        let result = self.brew_open(bottles, ingredient).await;
        self.close_station().await?;
        result
    }

    /// Vanilla order (2026-10-02 capture): bottles, ingredient, then fuel.
    async fn brew_open(&mut self, bottles: &[SlotRef], ingredient: &str) -> Result<(), ActionError> {
        let layout = if self.backend.bedrock().is_some() { BEDROCK } else { JAVA };
        for (&bottle, slot) in bottles.iter().zip(layout.bottles) {
            self.put(bottle, SlotRef::Container(slot), 1).await?;
        }
        self.put_matching(|name, _| name == ingredient, 1, SlotRef::Container(layout.ingredient)).await?;
        let fuel = SlotRef::Container(layout.fuel);
        if fuel.stack(&self.state).is_none_or(|s| s.is_empty()) {
            self.put_matching(|name, _| name == "minecraft:blaze_powder", 1, fuel).await?;
        }
        let input = SlotRef::Container(layout.ingredient);
        let brewed = |bot: &Bot, _: &RawPacket| input.stack(&bot.state).is_none_or(|s| s.is_empty()).then_some(());
        self.wait_until(BREW_TIMEOUT, brewed).await?;
        let results: Vec<SlotRef> = layout.bottles.iter().map(|&i| SlotRef::Container(i)).collect();
        self.take_back(&results).await
    }
}
