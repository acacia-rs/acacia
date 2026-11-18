//! `ClientboundBossEventPacket`: a UUID, an operation type as a VarInt, then that operation's body.

use crate::text::Component;
use crate::types::var_int_enum;
use crate::{Error, VarInt, Wire};

var_int_enum!(BossColor, "boss bar colour", [Pink, Blue, Red, Green, Yellow, Purple, White]);
var_int_enum!(BossOverlay, "boss bar overlay", [Progress, Notched6, Notched10, Notched12, Notched20]);

/// The three flag bits of the add and update-properties operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BossFlags {
    pub darken_screen: bool,
    pub play_music: bool,
    pub create_world_fog: bool,
}

impl Wire for BossFlags {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let bits = u8::read(input)?;
        Ok(Self { darken_screen: bits & 1 != 0, play_music: bits & 2 != 0, create_world_fog: bits & 4 != 0 })
    }

    fn write(&self, out: &mut Vec<u8>) {
        (u8::from(self.darken_screen) | u8::from(self.play_music) << 1 | u8::from(self.create_world_fog) << 2).write(out);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BossOperation {
    Add { name: Component, progress: f32, color: BossColor, overlay: BossOverlay, flags: BossFlags },
    Remove,
    UpdateProgress(f32),
    UpdateName(Component),
    UpdateStyle { color: BossColor, overlay: BossOverlay },
    UpdateProperties(BossFlags),
}

#[derive(Debug, Clone, PartialEq)]
pub struct BossEvent {
    pub id: u128,
    pub operation: BossOperation,
}

impl Wire for BossEvent {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let id = u128::read(input)?;
        let operation = match VarInt::read(input)?.0 {
            0 => BossOperation::Add {
                name: Component::read(input)?,
                progress: f32::read(input)?,
                color: BossColor::read(input)?,
                overlay: BossOverlay::read(input)?,
                flags: BossFlags::read(input)?,
            },
            1 => BossOperation::Remove,
            2 => BossOperation::UpdateProgress(f32::read(input)?),
            3 => BossOperation::UpdateName(Component::read(input)?),
            4 => BossOperation::UpdateStyle { color: BossColor::read(input)?, overlay: BossOverlay::read(input)? },
            5 => BossOperation::UpdateProperties(BossFlags::read(input)?),
            _ => return Err(Error::Invalid("boss event operation")),
        };
        Ok(Self { id, operation })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.id.write(out);
        match &self.operation {
            BossOperation::Add { name, progress, color, overlay, flags } => {
                VarInt(0).write(out);
                name.write(out);
                progress.write(out);
                color.write(out);
                overlay.write(out);
                flags.write(out);
            }
            BossOperation::Remove => VarInt(1).write(out),
            BossOperation::UpdateProgress(progress) => {
                VarInt(2).write(out);
                progress.write(out);
            }
            BossOperation::UpdateName(name) => {
                VarInt(3).write(out);
                name.write(out);
            }
            BossOperation::UpdateStyle { color, overlay } => {
                VarInt(4).write(out);
                color.write(out);
                overlay.write(out);
            }
            BossOperation::UpdateProperties(flags) => {
                VarInt(5).write(out);
                flags.write(out);
            }
        }
    }
}
