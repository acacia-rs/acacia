//! Entity movement: relative moves (`VecDelta`), absolute position sync, teleports, motion and head rotation.

use crate::entity::{LpVec3, PositionPath, read_path};
use crate::packets::{PositionMoveRotation, packet};
use crate::types::Vec3;
use crate::{Error, VarInt, Wire};

/// A relative move in 1/4096 block steps: one step over the receiver's interpolation, or steps of their own tick counts.
#[derive(Debug, Clone, PartialEq)]
pub enum VecDelta {
    Linear([i16; 3]),
    Stepped(Vec<(u32, [i16; 3])>),
}

impl Default for VecDelta {
    fn default() -> Self {
        Self::Linear([0; 3])
    }
}

impl VecDelta {
    fn step_count(&self) -> usize {
        match self {
            Self::Linear(_) => 0,
            Self::Stepped(steps) => steps.len(),
        }
    }

    fn write_body(&self, out: &mut Vec<u8>) {
        let write_delta = |delta: &[i16; 3], out: &mut Vec<u8>| delta.iter().for_each(|c| c.write(out));
        match self {
            Self::Linear(delta) => write_delta(delta, out),
            Self::Stepped(steps) => steps.iter().for_each(|(ticks, delta)| {
                VarInt(*ticks as i32).write(out);
                write_delta(delta, out);
            }),
        }
    }

    fn read_body(count: usize, input: &mut &[u8]) -> Result<Self, Error> {
        let read_delta = |input: &mut &[u8]| -> Result<[i16; 3], Error> { Ok([i16::read(input)?, i16::read(input)?, i16::read(input)?]) };
        if count == 0 {
            return read_delta(input).map(Self::Linear);
        }
        let mut steps = Vec::with_capacity(count.min(64));
        for _ in 0..count {
            steps.push((VarInt::read(input)?.0 as u32, read_delta(input)?));
        }
        Ok(Self::Stepped(steps))
    }
}

/// The properties VarInt of `MoveEntity.Pos`/`PosRot`: on ground in bit 0, the step count above it.
fn write_properties(on_ground: bool, delta: &VecDelta, out: &mut Vec<u8>) {
    VarInt((delta.step_count() << 1) as i32 | i32::from(on_ground)).write(out);
}

fn read_properties(input: &mut &[u8]) -> Result<(bool, usize), Error> {
    let properties = VarInt::read(input)?.0;
    Ok((properties & 1 != 0, ((properties as u32) >> 1) as usize))
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MoveEntityPos {
    pub entity: VarInt,
    pub on_ground: bool,
    pub delta: VecDelta,
}

impl Wire for MoveEntityPos {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let entity = VarInt::read(input)?;
        let (on_ground, count) = read_properties(input)?;
        Ok(Self { entity, on_ground, delta: VecDelta::read_body(count, input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.entity.write(out);
        write_properties(self.on_ground, &self.delta, out);
        self.delta.write_body(out);
    }
}

/// Rotations are `pack_degrees` bytes, y then x.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MoveEntityPosRot {
    pub entity: VarInt,
    pub on_ground: bool,
    pub delta: VecDelta,
    pub y_rot: u8,
    pub x_rot: u8,
}

impl Wire for MoveEntityPosRot {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let entity = VarInt::read(input)?;
        let (on_ground, count) = read_properties(input)?;
        Ok(Self { entity, on_ground, delta: VecDelta::read_body(count, input)?, y_rot: u8::read(input)?, x_rot: u8::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.entity.write(out);
        write_properties(self.on_ground, &self.delta, out);
        self.delta.write_body(out);
        self.y_rot.write(out);
        self.x_rot.write(out);
    }
}

packet!(MoveEntityRot { entity: VarInt, on_ground: bool, y_rot: u8, x_rot: u8 });
packet!(RotateHead { entity: VarInt, y_head_rot: u8 });
packet!(SetEntityMotion { entity: VarInt, movement: LpVec3 });
// `relatives` is `Relative`'s bit set as a big-endian int, bit = ordinal.
packet!(TeleportEntity { entity: VarInt, change: PositionMoveRotation, relatives: i32, on_ground: bool });

impl Default for PositionPath {
    fn default() -> Self {
        Self::Linear([0.0; 3])
    }
}

/// `PositionPath.STREAM_CODEC`: a type VarInt (linear 0, stepped 1), then one `Vec3`, or a list of `(Vec3, ticks)`.
impl Wire for PositionPath {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        read_path(input)
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Linear(end) => {
                VarInt(0).write(out);
                Vec3 { x: end[0], y: end[1], z: end[2] }.write(out);
            }
            Self::Stepped(steps) => {
                VarInt(1).write(out);
                VarInt(steps.len() as i32).write(out);
                for (at, ticks) in steps {
                    Vec3 { x: at[0], y: at[1], z: at[2] }.write(out);
                    VarInt(*ticks as i32).write(out);
                }
            }
        }
    }
}

packet!(EntityPositionSync { entity: VarInt, path: PositionPath, y_rot: f32, x_rot: f32, on_ground: bool });
