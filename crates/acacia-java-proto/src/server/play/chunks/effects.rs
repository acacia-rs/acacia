//! `LevelEvent`, `Sound`, `SoundEntity` and `StopSound`.

#![allow(unused_doc_comments)] // docs on `packet!` fields' structs

use super::{read_option, write_option};
use crate::actions::BlockPos;
use crate::packets::packet;
use crate::{Error, VarInt, Wire};

/// `type` is a `LevelEvent` constant; `global` plays it for the whole level instead of by distance.
packet!(LevelEvent { event: i32, pos: BlockPos, data: i32, global: bool });

/// `SoundSource` ordinals, written as VarInts. azalea lacks `Ui`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoundSource {
    #[default]
    Master,
    Music,
    Records,
    Weather,
    Blocks,
    Hostile,
    Neutral,
    Players,
    Ambient,
    Voice,
    Ui,
}

impl SoundSource {
    const ALL: [Self; 11] = [
        Self::Master,
        Self::Music,
        Self::Records,
        Self::Weather,
        Self::Blocks,
        Self::Hostile,
        Self::Neutral,
        Self::Players,
        Self::Ambient,
        Self::Voice,
        Self::Ui,
    ];
}

impl Wire for SoundSource {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let id = VarInt::read(input)?.0;
        usize::try_from(id).ok().and_then(|i| Self::ALL.get(i).copied()).ok_or(Error::Invalid("sound source"))
    }

    fn write(&self, out: &mut Vec<u8>) {
        VarInt(*self as i32).write(out);
    }
}

/// `SoundEvent.STREAM_CODEC`, a holder: 0, then the event inline, else the registry id plus one.
#[derive(Debug, Clone, PartialEq)]
pub enum SoundRef {
    /// A `sound_event` registry id.
    Id(i32),
    /// An event the registry lacks: its identifier and an optional fixed audible range.
    Inline { name: String, fixed_range: Option<f32> },
}

impl Default for SoundRef {
    fn default() -> Self {
        Self::Id(0)
    }
}

impl Wire for SoundRef {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        match VarInt::read(input)?.0 {
            0 => Ok(Self::Inline { name: String::read(input)?, fixed_range: read_option(input)? }),
            n => Ok(Self::Id(n - 1)),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Id(id) => VarInt(id + 1).write(out),
            Self::Inline { name, fixed_range } => {
                VarInt(0).write(out);
                name.write(out);
                write_option(fixed_range, out);
            }
        }
    }
}

/// Positions in `Sound` are fixed point, eighths of a block.
pub fn sound_coord(v: f64) -> i32 {
    (v * 8.0) as i32
}

packet!(Sound { sound: SoundRef, source: SoundSource, x: i32, y: i32, z: i32, volume: f32, pitch: f32, seed: i64 });
packet!(SoundEntity { sound: SoundRef, source: SoundSource, entity: VarInt, volume: f32, pitch: f32, seed: i64 });

/// `ClientboundStopSoundPacket`: a flags byte (1 source, 2 name), then each present field. None of either stops all.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StopSound {
    pub source: Option<SoundSource>,
    pub name: Option<String>,
}

impl Wire for StopSound {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let flags = u8::read(input)?;
        let source = if flags & 1 != 0 { Some(SoundSource::read(input)?) } else { None };
        let name = if flags & 2 != 0 { Some(String::read(input)?) } else { None };
        Ok(Self { source, name })
    }

    fn write(&self, out: &mut Vec<u8>) {
        (u8::from(self.source.is_some()) | u8::from(self.name.is_some()) << 1).write(out);
        if let Some(source) = self.source {
            source.write(out);
        }
        if let Some(name) = &self.name {
            name.write(out);
        }
    }
}
