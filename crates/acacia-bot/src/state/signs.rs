use acacia_client::proto::packets::{ChangeDimension, OpenSign};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

/// A sign editor the server opened (after placing a sign or clicking one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignEditor {
    pub position: [i32; 3],
    pub front: bool,
}

/// The open sign editor. Sign text lives in [`super::BlockEntities`].
#[derive(Debug, Default)]
pub struct Signs {
    pub editor: Option<SignEditor>,
}

impl Signs {
    pub const PACKETS: &'static [u32] = &[OpenSign::ID, ChangeDimension::ID];

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        match packet.id {
            OpenSign::ID => {
                let p: OpenSign = packet.decode()?;
                self.editor = Some(SignEditor { position: [p.position.x, p.position.y, p.position.z], front: p.is_front });
            }
            ChangeDimension::ID => self.editor = None,
            _ => {}
        }
        Ok(())
    }
}
