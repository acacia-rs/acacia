//! The sign editor's answer. The server opens the editor with `OpenSignEditor` after a sign is placed or clicked;
//! `SignEditScreen.removed` sends the side's four lines back at once, outside the tick.

use acacia_java_proto::actions::{BlockPos, SignUpdate};
use acacia_java_proto::ids::play::serverbound as to;

use crate::frame::Error;
use crate::join::Client;

impl Client {
    /// Sends one side of the sign at `pos`: the server takes it only from the player it opened the editor for.
    pub fn update_sign(&self, pos: [i32; 3], front: bool, lines: [String; 4]) -> Result<(), Error> {
        self.send(to::SIGN_UPDATE, &SignUpdate { pos: BlockPos(pos), lines, front })
    }
}
