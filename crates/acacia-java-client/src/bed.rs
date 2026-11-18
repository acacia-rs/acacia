//! The player in bed. The server's `SLEEPING_POS_ID` for the player puts it in the bed as
//! `LivingEntity.onSyncedDataUpdated` does (`setPosToBed`); the server ends sleep with an empty position and a
//! teleport to the stand-up spot. Grim doesn't simulate a player in bed, only keeps it within a block of it.

use acacia_java_proto::actions::BlockPos;
use acacia_java_proto::packets::COMMAND_STOP_SLEEPING;
use acacia_java_proto::{Error as WireError, Wire, ids, read_varint};
use acacia_registry::{Block, BlockState, BlockTag};

use crate::entity::seek_data;
use crate::frame::Error;
use crate::join::Client;
use crate::world::World;

use ids::play::clientbound as from;

/// `LivingEntity.SLEEPING_POS_ID`, an optional block pos (serializer 11).
const DATA_SLEEPING_POS: u8 = 14;
const SERIALIZER_OPTIONAL_BLOCK_POS: i32 = 11;
/// `setPosToBed`'s `adjustHeight` over the bed's sleep height.
const ADJUST_HEIGHT: f64 = 0.125;

impl Client {
    /// The bed the player sleeps in.
    pub fn bed(&self) -> Option<[i32; 3]> {
        self.movement.player.sleeping_pos
    }

    /// The bed screen's "Leave Bed" (`InBedChatScreen.sendWakeUp`); nothing while awake, as Grim's BadPacketsC
    /// flags a wake-up out of bed.
    pub fn leave_bed(&mut self) -> Result<(), Error> {
        if self.bed().is_none() {
            return Ok(());
        }
        self.send_command_with(COMMAND_STOP_SLEEPING, 0)
    }

    pub(crate) fn apply_bed(&mut self, id: i32, mut input: &[u8]) -> Result<(), WireError> {
        if id != from::SET_ENTITY_DATA || read_varint(&mut input)? != self.world.player_id {
            return Ok(());
        }
        if seek_data(&mut input, DATA_SLEEPING_POS)? != Some(SERIALIZER_OPTIONAL_BLOCK_POS) {
            return Ok(());
        }
        let bed = if bool::read(&mut input)? { Some(BlockPos::read(&mut input)?.0) } else { None };
        self.movement.player.sleeping_pos = bed;
        if let Some(spot) = bed.and_then(|pos| sleep_spot(&self.world, pos)) {
            self.movement.player.pos = spot;
        }
        Ok(())
    }

    /// The player stands on a bed's sleeping spot, where `ServerPlayer.startSleeping` teleports it (Paper twice)
    /// before the data that sets the bed goes out.
    pub fn at_sleep_spot(&self) -> bool {
        let pos = self.movement.player.pos;
        sleep_spot(&self.world, pos.map(|v| v.floor() as i32)).is_some_and(|spot| spot == pos)
    }
}

/// `setPosToBed`: where the player lies in the bed at `pos`.
fn sleep_spot(world: &World, [x, y, z]: [i32; 3]) -> Option<[f64; 3]> {
    let height = sleep_height(world, [x, y, z])?;
    Some([f64::from(x) + 0.5, f64::from(y) + height + ADJUST_HEIGHT, f64::from(z) + 0.5])
}

/// `AbstractBedBlock.getSleepHeight`: the top of the bed's shape, if `pos` holds a bed.
fn sleep_height(world: &World, pos: [i32; 3]) -> Option<f64> {
    let state = world.block(pos).and_then(BlockState::from_id)?;
    let block = state.block();
    if !(block.is(BlockTag::Beds) || block == Block::StrawBed) {
        return None;
    }
    state.java_collision()?.iter().map(|b| b[4]).reduce(f64::max)
}
