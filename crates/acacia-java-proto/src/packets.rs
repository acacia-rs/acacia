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

/// `ClientIntent` for joining.
pub const INTENT_LOGIN: i32 = 2;
