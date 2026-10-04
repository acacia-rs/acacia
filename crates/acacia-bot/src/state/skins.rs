use std::collections::HashMap;
use std::sync::Arc;

use acacia_client::proto::manual::Uuid;
use acacia_client::proto::packets::{PlayerList as PlayerListPacket, PlayerSkin as PlayerSkinPacket};
use acacia_client::proto::types::{PlayerRecordContent, Skin, SkinArmSize, SkinImage};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

/// RGBA8 pixels, `width * height * 4` bytes, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinTexture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl SkinTexture {
    /// `None` when the image doesn't match its size (persona skins without a baked texture).
    fn from_wire(image: SkinImage) -> Option<Self> {
        let (width, height) = (u32::try_from(image.width).ok()?, u32::try_from(image.height).ok()?);
        (width > 0 && image.data.len() == (width * height * 4) as usize).then(|| Self { width, height, rgba: image.data.to_vec() })
    }
}

/// A player's skin. Capes and body animations are dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSkin {
    pub texture: SkinTexture,
    /// Persona skins keep the face apart: frames stacked top to bottom, for the geometry the
    /// resource patch names `animated_face`.
    pub face: Option<SkinTexture>,
    /// 3-pixel arms (Alex) rather than 4.
    pub slim: bool,
    /// JSON naming the skin's geometry: `{"geometry": {"default": "geometry.humanoid.custom"}}`.
    pub resource_patch: String,
    /// Geometry file JSON defining those geometries, for persona and custom-model skins; often empty.
    pub geometry_data: String,
}

/// `SkinAnimationsItem::animation_type` of the face texture (gophertunnel `SkinAnimationHead`).
const ANIMATION_FACE: u32 = 1;

impl PlayerSkin {
    fn from_wire(skin: Skin) -> Option<Self> {
        let face = skin.animations.into_iter().find(|a| a.animation_type == ANIMATION_FACE);
        Some(Self {
            texture: SkinTexture::from_wire(skin.skin_data)?,
            face: face.and_then(|a| SkinTexture::from_wire(a.skin_image)),
            slim: skin.arm_size == SkinArmSize::Slim,
            resource_patch: skin.skin_resource_pack,
            geometry_data: skin.geometry_data,
        })
    }
}

/// Skins of the players in the tab list, by UUID. Off by default (see [`crate::BotConfig`]): a
/// skin is 16 KB to 256 KB per player, more with its own geometry.
#[derive(Debug, Default)]
pub struct Skins {
    by_uuid: HashMap<Uuid, Arc<PlayerSkin>>,
}

impl Skins {
    pub const PACKETS: &'static [u32] = &[PlayerListPacket::ID, PlayerSkinPacket::ID];

    pub fn get(&self, uuid: &Uuid) -> Option<&Arc<PlayerSkin>> {
        self.by_uuid.get(uuid)
    }

    pub fn len(&self) -> usize {
        self.by_uuid.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_uuid.is_empty()
    }

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        if packet.id == PlayerSkinPacket::ID {
            let p: PlayerSkinPacket = packet.decode()?;
            self.set(p.uuid, p.skin);
            return Ok(());
        }
        for record in packet.decode::<PlayerListPacket>()?.records {
            match record.content {
                PlayerRecordContent::Add(add) => self.set(add.uuid, add.skin_data),
                PlayerRecordContent::Remove(r) => {
                    self.by_uuid.remove(&r.uuid);
                }
                PlayerRecordContent::Default => {}
            }
        }
        Ok(())
    }

    fn set(&mut self, uuid: Uuid, skin: Skin) {
        match PlayerSkin::from_wire(skin) {
            Some(skin) => self.by_uuid.insert(uuid, Arc::new(skin)),
            None => self.by_uuid.remove(&uuid),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::queries::test_support::{fixtures, raw};

    #[test]
    fn keeps_skins_from_the_player_list_and_drops_them_on_remove() {
        let list = fixtures::<PlayerListPacket>();
        let mut record = list
            .iter()
            .flat_map(|raw| raw.decode::<PlayerListPacket>().unwrap().records)
            .find(|r| matches!(r.content, PlayerRecordContent::Add(_)))
            .expect("add record in fixtures");
        let PlayerRecordContent::Add(add) = &mut record.content else { unreachable!() };
        add.uuid = Uuid([7; 16]);
        (add.skin_data.skin_data.width, add.skin_data.skin_data.height) = (2, 1);
        add.skin_data.skin_data.data = vec![1, 2, 3, 4, 5, 6, 7, 8].into();
        add.skin_data.arm_size = SkinArmSize::Slim;

        let mut skins = Skins::default();
        skins.apply(&raw(&PlayerListPacket { records: vec![record.clone()] })).unwrap();
        let skin = skins.get(&Uuid([7; 16])).unwrap();
        assert_eq!((skin.texture.width, skin.texture.height, skin.slim, skin.texture.rgba.len()), (2, 1, true, 8));

        let PlayerRecordContent::Add(add) = &mut record.content else { unreachable!() };
        add.skin_data.skin_data.width = 64;
        skins.apply(&raw(&PlayerListPacket { records: vec![record] })).unwrap();
        assert!(skins.is_empty(), "an image that doesn't match its size is dropped");
    }
}
