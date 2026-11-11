//! Digging, using and attacking, sequenced as `MultiPlayerGameMode` does. When to dig on, and for how long, is the
//! caller's: the client only sends what vanilla would send at each step. Actions wait for the next
//! [`Client::step`], which sends them first, as vanilla's `handleKeybinds` runs after the tick's packets are
//! handled and before the player moves: Grim's Post check flags an action that reaches it before a pong.

use acacia_java_proto::actions::{Action, Attack, BlockChangedAck, BlockPos, Direction, Hand, Interact, PlayerAction, SetCarriedItem, UseItem, UseItemOn};
use acacia_java_proto::entity::LpVec3;
use acacia_java_proto::ids::play::{clientbound as from, serverbound as to};
use acacia_java_proto::packets::Empty;
use acacia_java_proto::{Error as WireError, VarInt, Wire, read_varint};

use crate::frame::Error;
use crate::join::Client;

const AIR: u32 = 0;

/// Block-action bookkeeping: the prediction sequence and the hotbar slot the server knows.
#[derive(Debug, Default)]
pub struct Actions {
    /// The slot to act with; sent before the next action when it changed.
    pub selected: u8,
    carried: u8,
    sequence: i32,
    acked: i32,
    outbox: Vec<(i32, Vec<u8>)>,
}

impl Actions {
    /// The newest action the server has answered, by sequence; compare with [`Client::start_destroy`]'s result.
    pub fn acked(&self) -> i32 {
        self.acked
    }

    pub(crate) fn apply(&mut self, id: i32, mut body: &[u8]) -> Result<(), WireError> {
        match id {
            from::BLOCK_CHANGED_ACK => self.acked = self.acked.max(BlockChangedAck::read(&mut body)?.sequence.0),
            from::SET_HELD_SLOT => self.selected = read_varint(&mut body)? as u8,
            _ => {}
        }
        Ok(())
    }

    /// `BlockStatePredictionHandler.startPredicting`: bump, then use.
    fn next_sequence(&mut self) -> i32 {
        self.sequence += 1;
        self.sequence
    }
}

/// A body already encoded.
struct Encoded<'a>(&'a [u8]);

impl Wire for Encoded<'_> {
    fn read(_: &mut &[u8]) -> Result<Self, WireError> {
        unreachable!("only written")
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(self.0);
    }
}

impl Client {
    pub(crate) fn queue(&mut self, id: i32, body: &impl Wire) -> Result<(), Error> {
        let mut bytes = Vec::new();
        body.write(&mut bytes);
        self.actions.outbox.push((id, bytes));
        Ok(())
    }

    /// Sends the queued actions, after the held slot if it changed.
    pub(crate) fn send_actions(&mut self) -> Result<(), Error> {
        self.sync_carried()?;
        for (id, body) in std::mem::take(&mut self.actions.outbox) {
            self.send(id, &Encoded(&body))?;
        }
        Ok(())
    }

    /// `ensureHasSentCarriedItem`, which vanilla runs at the start of each tick and before every action.
    pub(crate) fn sync_carried(&mut self) -> Result<(), Error> {
        if self.actions.selected != self.actions.carried {
            self.actions.carried = self.actions.selected;
            self.queue(to::SET_CARRIED_ITEM, &SetCarriedItem { slot: i16::from(self.actions.carried) })?;
        }
        Ok(())
    }

    fn block_action(&mut self, action: Action, pos: [i32; 3], face: Direction, sequenced: bool) -> Result<i32, Error> {
        let sequence = if sequenced { self.actions.next_sequence() } else { 0 };
        self.queue(to::PLAYER_ACTION, &PlayerAction { action, pos: BlockPos(pos), face, sequence })?;
        Ok(sequence)
    }

    /// Starts digging `pos`, or in creative breaks it. `breaks` predicts air, as vanilla does when the block's progress
    /// per tick reaches 1 or the player is in creative. Returns the action's sequence.
    pub fn start_destroy(&mut self, pos: [i32; 3], face: Direction, breaks: bool) -> Result<i32, Error> {
        self.sync_carried()?;
        let sequence = self.block_action(Action::StartDestroy, pos, face, true)?;
        if breaks {
            self.world.set_block(pos, AIR);
        }
        Ok(sequence)
    }

    /// Finishes a dig whose progress reached 1, predicting air. Returns the action's sequence.
    pub fn stop_destroy(&mut self, pos: [i32; 3], face: Direction) -> Result<i32, Error> {
        self.sync_carried()?;
        let sequence = self.block_action(Action::StopDestroy, pos, face, true)?;
        self.world.set_block(pos, AIR);
        Ok(sequence)
    }

    /// Gives up a dig: vanilla sends `Direction::Down` when the player lets go.
    pub fn abort_destroy(&mut self, pos: [i32; 3], face: Direction) -> Result<(), Error> {
        self.block_action(Action::AbortDestroy, pos, face, false).map(drop)
    }

    pub fn change_destroy_direction(&mut self, pos: [i32; 3], face: Direction) -> Result<(), Error> {
        self.block_action(Action::ChangeDestroyDirection, pos, face, false).map(drop)
    }

    /// The arm swing: once per attack, and every tick a dig goes on.
    pub fn punch(&mut self) -> Result<(), Error> {
        self.queue(to::PUNCH, &Empty {})
    }

    /// Clicks `face` of `pos` at `click` (relative to the block's corner). Returns the action's sequence.
    pub fn use_item_on(&mut self, hand: Hand, pos: [i32; 3], face: Direction, click: [f32; 3]) -> Result<i32, Error> {
        self.sync_carried()?;
        let sequence = self.actions.next_sequence();
        let packet = UseItemOn { hand, pos: BlockPos(pos), face, click, inside: false, world_border: false, sequence };
        self.queue(to::USE_ITEM_ON, &packet)?;
        Ok(sequence)
    }

    /// Uses the held item in the air, facing where the player looks. Returns the action's sequence.
    pub fn use_item(&mut self, hand: Hand) -> Result<i32, Error> {
        self.sync_carried()?;
        let sequence = self.actions.next_sequence();
        let player = &self.movement.player;
        let packet = UseItem { hand, sequence: VarInt(sequence), y_rot: player.yaw, x_rot: player.pitch };
        self.queue(to::USE_ITEM, &packet)?;
        Ok(sequence)
    }

    /// Lets go of an item being used, such as a drawn bow.
    pub fn release_use_item(&mut self) -> Result<(), Error> {
        self.sync_carried()?;
        self.block_action(Action::ReleaseUseItem, [0; 3], Direction::Down, false).map(drop)
    }

    /// Hits `entity`; vanilla follows with [`Client::punch`].
    pub fn attack(&mut self, entity: i32) -> Result<(), Error> {
        self.sync_carried()?;
        self.queue(to::ATTACK, &Attack { entity: VarInt(entity) })
    }

    /// Right-clicks `entity` at `location`, relative to its feet; `secondary` is sneaking.
    pub fn interact(&mut self, entity: i32, hand: Hand, location: [f64; 3], secondary: bool) -> Result<(), Error> {
        self.sync_carried()?;
        self.queue(to::INTERACT, &Interact { entity: VarInt(entity), hand, location: LpVec3(location), secondary })
    }
}
