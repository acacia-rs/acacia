//! Runs the local player's physics and tells the server, as `LocalPlayer.sendChanges` does.

use acacia_java_physics::{Blocks, Keys, Player};
use acacia_java_proto::packets::{
    COMMAND_START_FALL_FLYING, COMMAND_START_SPRINTING, COMMAND_STOP_SPRINTING, ClientboundAbilities, Empty, MovePlayerPos, MovePlayerPosRot, MovePlayerRot, MovePlayerStatusOnly,
    PlayerCommand, PlayerInput, PlayerPosition, ServerboundAbilities,
};
use acacia_java_proto::entity::read_lp_vec3;
use acacia_java_proto::{Error as WireError, VarInt, Wire, ids, read_varint};

use crate::frame::Error;
use crate::join::{Client, Pose};
use crate::riding::Riding;
use crate::world::World;

use ids::play::{clientbound as from, serverbound as to};

const ABILITY_FLYING: u8 = 2;
const ABILITY_MAY_FLY: u8 = 4;
const ATTRIBUTE_MOVEMENT_SPEED: i32 = 26;
const DATA_SHARED_FLAGS: u8 = 0;
const SERIALIZER_BYTE: i32 = 0;
const FLAG_FALL_FLYING: u8 = 1 << 7;
const OPERATION_ADD_VALUE: i32 = 0;
const MIN_MOVE_SQUARED: f64 = 2.0E-4 * 2.0E-4;
const POSITION_REMINDER_TICKS: u32 = 20;

impl Blocks for World {
    fn state(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        self.block([x, y, z])
    }

    fn min_y(&self) -> i32 {
        World::min_y(self)
    }
}

/// What the server was last told, next to the simulated player.
pub struct Movement {
    pub player: Player,
    /// The seat while the player rides; the player's own physics pauses meanwhile.
    pub riding: Option<Riding>,
    /// Server corrections of driven vehicles since joining: [height only, horizontal].
    pub vehicle_corrections: [u32; 2],
    sent_keys: Keys,
    sent_sprinting: bool,
    sent_flying: bool,
    sent_gliding: bool,
    sent_pos: [f64; 3],
    sent_rot: [f32; 2],
    sent_on_ground: bool,
    sent_collision: bool,
    position_reminder: u32,
}

impl Default for Movement {
    fn default() -> Self {
        Self {
            player: Player::at([0.0; 3]),
            riding: None,
            vehicle_corrections: [0; 2],
            sent_keys: Keys::default(),
            sent_sprinting: false,
            sent_flying: false,
            sent_gliding: false,
            sent_pos: [0.0; 3],
            sent_rot: [0.0; 2],
            sent_on_ground: false,
            sent_collision: false,
            position_reminder: 0,
        }
    }
}

impl Movement {
    /// `setValuesFromPositionPacket`; `pose` is the position already made absolute.
    // TODO: Relative.ROTATE_DELTA (bit 8) is not applied.
    pub(crate) fn teleported(&mut self, pose: Pose, packet: &PlayerPosition) {
        let c = &packet.change;
        let change = [c.delta_x, c.delta_y, c.delta_z];
        let delta = std::array::from_fn(|a| if packet.relatives & (1 << (5 + a)) != 0 { self.player.delta[a] } else { 0.0 } + change[a]);
        self.player.pos = pose.position;
        self.player.delta = delta;
        self.player.yaw = pose.y_rot;
        self.player.pitch = pose.x_rot;
    }

    pub(crate) fn apply(&mut self, id: i32, mut input: &[u8], player_id: i32) -> Result<(), WireError> {
        if id == from::SET_ENTITY_MOTION && read_varint(&mut input)? == player_id {
            self.player.delta = read_lp_vec3(&mut input)?;
        } else if id == from::SET_ENTITY_DATA && read_varint(&mut input)? == player_id {
            self.read_shared_flags(&mut input)?;
        } else if id == from::UPDATE_ATTRIBUTES && read_varint(&mut input)? == player_id {
            self.read_attributes(&mut input)?;
        } else if id == from::PLAYER_ABILITIES {
            let abilities = ClientboundAbilities::read(&mut input)?;
            self.player.flying = abilities.flags & ABILITY_FLYING != 0;
            self.player.may_fly = abilities.flags & ABILITY_MAY_FLY != 0;
            self.player.flying_speed = abilities.flying_speed;
            self.sent_flying = self.player.flying;
        }
        Ok(())
    }

    /// Only the server ends a glide: the client glides on, landed or not, until the shared flags say otherwise.
    /// Items are sent in index order, so the flags byte (index 0) can only come first.
    fn read_shared_flags(&mut self, input: &mut &[u8]) -> Result<(), WireError> {
        if u8::read(input)? == DATA_SHARED_FLAGS && read_varint(input)? == SERIALIZER_BYTE {
            self.player.fall_flying = u8::read(input)? & FLAG_FALL_FLYING != 0;
        }
        Ok(())
    }

    /// Only flat movement-speed modifiers matter here (powder snow's freeze); sprint and potions are simulated.
    fn read_attributes(&mut self, input: &mut &[u8]) -> Result<(), WireError> {
        for _ in 0..read_varint(input)? {
            let attribute = read_varint(input)?;
            f64::read(input)?;
            let mut flat = 0.0;
            for _ in 0..read_varint(input)? {
                String::read(input)?;
                let amount = f64::read(input)?;
                if read_varint(input)? == OPERATION_ADD_VALUE {
                    flat += amount;
                }
            }
            if attribute == ATTRIBUTE_MOVEMENT_SPEED {
                self.player.speed_add = flat;
            }
        }
        Ok(())
    }

    fn collision_flags(&self) -> u8 {
        u8::from(self.player.on_ground) | u8::from(self.player.horizontal_collision) << 1
    }
}

fn input_flags(k: Keys) -> u8 {
    [k.forward, k.backward, k.left, k.right, k.jump, k.shift, k.sprint].iter().enumerate().fold(0, |flags, (bit, on)| flags | u8::from(*on) << bit)
}

impl Client {
    /// One client tick: simulates the player with `keys` and sends what changed. Does nothing before the first
    /// teleport or while the player's chunk is missing.
    pub fn step(&mut self, keys: Keys) -> Result<(), Error> {
        self.send_actions()?;
        self.world.tick_entities();
        let at = self.movement.player.pos.map(|v| v.floor() as i32);
        if !self.loaded() || !self.world.has_chunk(at) {
            return Ok(());
        }
        if self.movement.riding.is_some() {
            self.ride_step(keys)?;
        } else {
            self.movement.player.solid_entities = self.world.collision_boxes(false, &[]);
            self.movement.player.tick(&self.world, keys);
            self.send_changes()?;
            let me = self.world.player_id;
            for _ in self.world.entities.values().filter(|e| e.attached_to == Some(me)) {
                self.movement.player.firework_boost();
            }
        }
        let player = &self.movement.player;
        self.pose.position = player.pos;
        self.pose.y_rot = player.yaw;
        self.pose.x_rot = player.pitch;
        self.send(to::CLIENT_TICK_END, &Empty {})
    }

    fn send_command(&mut self, action: i32) -> Result<(), Error> {
        let packet = PlayerCommand { entity: VarInt(self.world.player_id), action: VarInt(action), data: VarInt(0) };
        self.send(to::PLAYER_COMMAND, &packet)
    }

    fn send_changes(&mut self) -> Result<(), Error> {
        let (flying, gliding) = (self.movement.player.flying, self.movement.player.fall_flying);
        // Vanilla sends these two from aiStep, so they arrive before the tick's input; Grim's ElytraB relies on it.
        if flying != self.movement.sent_flying {
            self.send(to::PLAYER_ABILITIES, &ServerboundAbilities { flags: if flying { ABILITY_FLYING } else { 0 } })?;
            self.movement.sent_flying = flying;
        }
        if gliding && !self.movement.sent_gliding {
            self.send_command(COMMAND_START_FALL_FLYING)?;
        }
        self.movement.sent_gliding = gliding;
        self.send_keys()?;
        self.send_sprint()?;
        self.send_position()
    }

    /// `PlayerInput` when the keys changed.
    pub(crate) fn send_keys(&mut self) -> Result<(), Error> {
        let keys = self.movement.player.keys;
        if keys != self.movement.sent_keys {
            self.send(to::PLAYER_INPUT, &PlayerInput { flags: input_flags(keys) })?;
            self.movement.sent_keys = keys;
        }
        Ok(())
    }

    pub(crate) fn send_sprint(&mut self) -> Result<(), Error> {
        let sprinting = self.movement.player.sprinting;
        if sprinting != self.movement.sent_sprinting {
            self.send_command(if sprinting { COMMAND_START_SPRINTING } else { COMMAND_STOP_SPRINTING })?;
            self.movement.sent_sprinting = sprinting;
        }
        Ok(())
    }

    fn send_position(&mut self) -> Result<(), Error> {
        let m = &mut self.movement;
        let [x, y, z] = m.player.pos;
        let (y_rot, x_rot) = (m.player.yaw, m.player.pitch);
        let flags = m.collision_flags();
        let moved: f64 = (0..3).map(|a| (m.player.pos[a] - m.sent_pos[a]).powi(2)).sum();
        m.position_reminder += 1;
        let send_pos = moved > MIN_MOVE_SQUARED || m.position_reminder >= POSITION_REMINDER_TICKS;
        let send_rot = m.sent_rot != [y_rot, x_rot];
        let status_changed = m.sent_on_ground != m.player.on_ground || m.sent_collision != m.player.horizontal_collision;
        if send_pos {
            m.sent_pos = m.player.pos;
            m.position_reminder = 0;
        }
        m.sent_rot = [y_rot, x_rot];
        m.sent_on_ground = m.player.on_ground;
        m.sent_collision = m.player.horizontal_collision;
        match (send_pos, send_rot) {
            (true, true) => self.send(to::MOVE_PLAYER_POS_ROT, &MovePlayerPosRot { x, y, z, y_rot, x_rot, flags }),
            (true, false) => self.send(to::MOVE_PLAYER_POS, &MovePlayerPos { x, y, z, flags }),
            (false, true) => self.send(to::MOVE_PLAYER_ROT, &MovePlayerRot { y_rot, x_rot, flags }),
            (false, false) if status_changed => self.send(to::MOVE_PLAYER_STATUS_ONLY, &MovePlayerStatusOnly { flags }),
            (false, false) => Ok(()),
        }
    }
}
