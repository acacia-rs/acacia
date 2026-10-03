//! Furnace, blast furnace and smoker: plain container slots (ingredient 0, fuel 1, output 2). The
//! server grants the smelting XP when the output is taken; the request carries nothing for it.

use std::time::Duration;

use acacia_client::proto::types::WindowType;
use acacia_client::proto::RawPacket;
use acacia_physics::BlockPos;

use crate::items::SlotRef;
use crate::{ActionError, Bot};

const INPUT: u8 = 0;
const FUEL: u8 = 1;
const OUTPUT: u8 = 2;
/// Waiting slack on top of the smelting time.
const SMELT_SLACK: Duration = Duration::from_secs(5);

/// Vanilla smelting time per item: 10 s in a furnace, 5 s in a blast furnace or smoker.
fn smelt_time(kind: WindowType) -> Duration {
    match kind {
        WindowType::Furnace => Duration::from_secs(10),
        _ => Duration::from_secs(5),
    }
}

impl Bot {
    /// Smelts `count` items named `input` at the furnace, blast furnace or smoker at `pos`: puts
    /// them in, adds `fuel` (identifier and count) if given, waits until they are done, takes the
    /// output and closes. Returns how many items were taken out.
    pub async fn smelt(&mut self, pos: BlockPos, input: &str, count: u8, fuel: Option<(&str, u8)>) -> Result<u16, ActionError> {
        self.open_station_and_look(pos, &[WindowType::Furnace, WindowType::BlastFurnace, WindowType::Smoker]).await?;
        let result = self.smelt_open(input, count, fuel).await;
        self.close_station().await?;
        result
    }

    async fn smelt_open(&mut self, input: &str, count: u8, fuel: Option<(&str, u8)>) -> Result<u16, ActionError> {
        let kind = self.state.containers.open.as_ref().map_or(WindowType::Furnace, |c| c.window_type);
        self.put_matching(|name, _| name == input, u16::from(count), SlotRef::Container(INPUT)).await?;
        if let Some((fuel, fuel_count)) = fuel {
            self.put_matching(|name, _| name == fuel, u16::from(fuel_count), SlotRef::Container(FUEL)).await?;
        }
        let timeout = smelt_time(kind) * u32::from(count) + SMELT_SLACK;
        let done = |bot: &Bot, _: &RawPacket| {
            let slot = |i| SlotRef::Container(i).stack(&bot.state).map_or(0, |s| s.count);
            (slot(INPUT) == 0 && slot(OUTPUT) > 0).then_some(())
        };
        match self.wait_until(timeout, done).await {
            Ok(()) | Err(ActionError::Timeout) => {}
            Err(e) => return Err(e),
        }
        let taken = SlotRef::Container(OUTPUT).stack(&self.state).map_or(0, |s| s.count);
        if taken == 0 {
            return Err(ActionError::Timeout);
        }
        self.take_back(&[SlotRef::Container(OUTPUT)]).await?;
        Ok(taken)
    }
}
