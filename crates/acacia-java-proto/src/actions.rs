//! What a player does to blocks, items and entities: digging, using, attacking, plus the server's acknowledgement.

use crate::entity::LpVec3;
use crate::packets::packet;
use crate::{Error, VarInt, Wire};

/// `BlockPos.asLong`: x 26 bits, z 26, y 12.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockPos(pub [i32; 3]);

impl Wire for BlockPos {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let v = i64::read(input)?;
        Ok(Self([(v >> 38) as i32, (v << 52 >> 52) as i32, (v << 26 >> 38) as i32]))
    }

    fn write(&self, out: &mut Vec<u8>) {
        let [x, y, z] = self.0.map(i64::from);
        ((x & 0x3ff_ffff) << 38 | (z & 0x3ff_ffff) << 12 | (y & 0xfff)).write(out);
    }
}

/// `Direction`'s 3D value: down, up, north, south, west, east.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Direction {
    #[default]
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    const ALL: [Self; 6] = [Self::Down, Self::Up, Self::North, Self::South, Self::West, Self::East];

    pub fn from_index(index: i32) -> Result<Self, Error> {
        usize::try_from(index).ok().and_then(|i| Self::ALL.get(i).copied()).ok_or(Error::Invalid("direction"))
    }
}

/// `InteractionHand`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Hand {
    #[default]
    Main,
    Off,
}

impl Wire for Hand {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        match VarInt::read(input)?.0 {
            0 => Ok(Self::Main),
            1 => Ok(Self::Off),
            _ => Err(Error::Invalid("hand")),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        VarInt(*self as i32).write(out);
    }
}

/// `ServerboundPlayerActionPacket.Action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Action {
    #[default]
    StartDestroy,
    ChangeDestroyDirection,
    AbortDestroy,
    StopDestroy,
    DropAll,
    Drop,
    ReleaseUseItem,
    SwapItemWithOffhand,
}

impl Action {
    const ALL: [Self; 8] = [
        Self::StartDestroy,
        Self::ChangeDestroyDirection,
        Self::AbortDestroy,
        Self::StopDestroy,
        Self::DropAll,
        Self::Drop,
        Self::ReleaseUseItem,
        Self::SwapItemWithOffhand,
    ];
}

/// `ServerboundPlayerActionPacket`: the direction goes as a byte here, unlike `UseItemOn`'s varint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerAction {
    pub action: Action,
    pub pos: BlockPos,
    pub face: Direction,
    pub sequence: i32,
}

impl Wire for PlayerAction {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let action = usize::try_from(VarInt::read(input)?.0).ok().and_then(|i| Action::ALL.get(i).copied());
        let action = action.ok_or(Error::Invalid("player action"))?;
        let pos = BlockPos::read(input)?;
        let face = Direction::from_index(i32::from(u8::read(input)?))?;
        Ok(Self { action, pos, face, sequence: VarInt::read(input)?.0 })
    }

    fn write(&self, out: &mut Vec<u8>) {
        VarInt(self.action as i32).write(out);
        self.pos.write(out);
        (self.face as u8).write(out);
        VarInt(self.sequence).write(out);
    }
}

/// `ServerboundUseItemOnPacket` with its `BlockHitResult`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UseItemOn {
    pub hand: Hand,
    pub pos: BlockPos,
    pub face: Direction,
    /// Where the ray hit, relative to the block's corner.
    pub click: [f32; 3],
    pub inside: bool,
    pub world_border: bool,
    pub sequence: i32,
}

impl Wire for UseItemOn {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let (hand, pos) = (Hand::read(input)?, BlockPos::read(input)?);
        let face = Direction::from_index(VarInt::read(input)?.0)?;
        let click = [f32::read(input)?, f32::read(input)?, f32::read(input)?];
        let (inside, world_border) = (bool::read(input)?, bool::read(input)?);
        Ok(Self { hand, pos, face, click, inside, world_border, sequence: VarInt::read(input)?.0 })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.hand.write(out);
        self.pos.write(out);
        VarInt(self.face as i32).write(out);
        self.click.iter().for_each(|c| c.write(out));
        self.inside.write(out);
        self.world_border.write(out);
        VarInt(self.sequence).write(out);
    }
}

packet!(UseItem { hand: Hand, sequence: VarInt, y_rot: f32, x_rot: f32 });
packet!(SetCarriedItem { slot: i16 });
packet!(Attack { entity: VarInt });
packet!(Interact { entity: VarInt, hand: Hand, location: LpVec3, secondary: bool });
packet!(BlockChangedAck { sequence: VarInt });

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T: Wire + PartialEq + std::fmt::Debug>(value: T) {
        let mut buf = Vec::new();
        value.write(&mut buf);
        assert_eq!(T::read(&mut &buf[..]), Ok(value));
    }

    #[test]
    fn positions_keep_their_sign() {
        round_trip(BlockPos([-5, -64, 77]));
        round_trip(BlockPos([33_554_431, 2047, -33_554_432]));
    }

    #[test]
    fn actions_round_trip() {
        round_trip(PlayerAction { action: Action::StopDestroy, pos: BlockPos([1, -2, 3]), face: Direction::East, sequence: 7 });
        let use_on = UseItemOn { face: Direction::Up, click: [0.5, 1.0, 0.25], sequence: 300, ..Default::default() };
        round_trip(use_on);
        round_trip(Interact { entity: VarInt(9), hand: Hand::Off, location: LpVec3([0.0; 3]), secondary: true });
    }

    #[test]
    fn player_action_face_is_one_byte() {
        let mut buf = Vec::new();
        PlayerAction { face: Direction::South, ..Default::default() }.write(&mut buf);
        assert_eq!(buf, [0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0]);
    }
}
