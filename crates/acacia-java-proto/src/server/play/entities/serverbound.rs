//! Serverbound entity interactions. Movement, input, commands, abilities and teleport acks are in
//! [`crate::packets`] (`MovePlayerPos`, `PlayerInput`, `PlayerCommand`, `MoveVehicle`, ...).

use crate::actions::Hand;
use crate::entity::LpVec3;
use crate::packets::packet;
use crate::{Error, VarInt, Wire};

// `location` is the hit point relative to the entity, in `LpVec3`'s encoding.
packet!(Interact { entity: VarInt, hand: Hand, location: LpVec3, using_secondary_action: bool });
packet!(Attack { entity: VarInt });
// Empty body.
packet!(Punch {});
// `y_rot`/`x_rot` are the look direction when the item was used.
packet!(UseItem { hand: Hand, sequence: VarInt, y_rot: f32, x_rot: f32 });
packet!(SetCarriedItem { slot: i16 });
packet!(PickItemFromEntity { entity: VarInt, include_data: bool });
packet!(TeleportToEntity { uuid: u128 });
