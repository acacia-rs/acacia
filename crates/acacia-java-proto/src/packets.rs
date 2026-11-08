//! Packet bodies. Ids live in [`crate::ids`], since a body like `KeepAlive` is shared by several protocol states.

use crate::{Error, VarInt, Wire};

macro_rules! packet {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Default)]
        pub struct $name { $(pub $field: $ty),* }

        impl Wire for $name {
            #[allow(unused_variables)]
            fn read(input: &mut &[u8]) -> Result<Self, Error> {
                Ok(Self { $($field: <$ty>::read(input)?),* })
            }
            #[allow(unused_variables)]
            fn write(&self, out: &mut Vec<u8>) {
                $(self.$field.write(out);)*
            }
        }
    };
}

packet!(Intention { protocol: VarInt, host: String, port: u16, intent: VarInt });
packet!(LoginHello { name: String, uuid: u128 });
packet!(LoginCompression { threshold: VarInt });
packet!(EncryptionHello { server_id: String, public_key: Vec<u8>, challenge: Vec<u8>, should_authenticate: bool });
packet!(EncryptionKey { secret: Vec<u8>, challenge: Vec<u8> });
// The profile's properties follow; nothing reads them yet.
packet!(LoginFinished { uuid: u128, name: String });
packet!(Empty {});
packet!(KeepAlive { id: i64 });
packet!(Ping { id: i32 });
packet!(KnownPack { namespace: String, id: String, version: String });
packet!(SelectKnownPacks { packs: Vec<KnownPack> });
packet!(PositionMoveRotation { x: f64, y: f64, z: f64, delta_x: f64, delta_y: f64, delta_z: f64, y_rot: f32, x_rot: f32 });
packet!(PlayerPosition { id: VarInt, change: PositionMoveRotation, relatives: i32 });
packet!(AcceptTeleportation { id: VarInt, x: f64, y: f64, z: f64, y_rot: f32, x_rot: f32 });
packet!(ChunkBatchReceived { chunks_per_tick: f32 });
// `flags` on the move packets: 1 on ground, 2 horizontal collision.
packet!(MovePlayerPos { x: f64, y: f64, z: f64, flags: u8 });
packet!(MovePlayerPosRot { x: f64, y: f64, z: f64, y_rot: f32, x_rot: f32, flags: u8 });
packet!(MovePlayerRot { y_rot: f32, x_rot: f32, flags: u8 });
packet!(MovePlayerStatusOnly { flags: u8 });
// Bits in `Input` order: forward, backward, left, right, jump, shift, sprint.
packet!(PlayerInput { flags: u8 });
packet!(PlayerCommand { entity: VarInt, action: VarInt, data: VarInt });
packet!(ServerboundAbilities { flags: u8 });
packet!(ClientboundAbilities { flags: u8, flying_speed: f32, walking_speed: f32 });

/// `ServerboundPlayerCommandPacket.Action` ordinals.
pub const COMMAND_START_SPRINTING: i32 = 1;
pub const COMMAND_STOP_SPRINTING: i32 = 2;
pub const COMMAND_START_FALL_FLYING: i32 = 6;

/// `ClientIntent` for joining.
pub const INTENT_LOGIN: i32 = 2;
