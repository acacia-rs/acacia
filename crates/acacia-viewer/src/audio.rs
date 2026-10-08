//! Sounds: the bot thread turns sound packets into [`Cue`]s, the window plays them from where the
//! camera is (acacia-sound).

use acacia_bot::Bot;
use acacia_bot::proto::packets::{LevelEvent, LevelEventEvent, LevelSoundEvent, PlaySound};
use acacia_bot::proto::{Packet, RawPacket};
use acacia_render::Camera;
use acacia_render::assets::Pack;
use acacia_sound::{Event, Listener, Sounds};

use crate::looks::Looks;
use crate::settings::LookChoice;

/// Packets that make sounds.
pub const PACKETS: [u32; 3] = [LevelSoundEvent::ID, PlaySound::ID, LevelEvent::ID];

/// A sound to play, with names already resolved on the bot thread.
#[derive(Debug, Clone)]
pub enum Cue {
    Named { name: String, at: Option<[f64; 3]>, volume: f32, pitch: f32 },
    Block { block: String, event: String, at: [f64; 3] },
    Entity { kind: String, event: String, at: [f64; 3] },
    World { event: String, at: [f64; 3] },
}

pub fn cue(bot: &Bot, packet: &RawPacket) -> Option<Cue> {
    let at = |p: acacia_bot::proto::types::Vec3f| [f64::from(p.x), f64::from(p.y), f64::from(p.z)];
    match packet.id {
        PlaySound::ID => {
            let p: PlaySound = packet.decode().ok()?;
            // Coordinates come in eighths of a block.
            let c = p.coordinates;
            let at = (!p.bypass_listener_range_check).then(|| [c.x, c.y, c.z].map(|v| f64::from(v) / 8.0));
            Some(Cue::Named { name: p.name, at, volume: p.volume, pitch: p.pitch })
        }
        LevelSoundEvent::ID => {
            let p: LevelSoundEvent = packet.decode().ok()?;
            let kind = p.entity_type.trim_start_matches(':');
            match block_name(bot, p.extra_data) {
                Some(block) if kind.is_empty() || kind == "minecraft:player" => Some(Cue::Block { block, event: p.sound_id, at: at(p.position) }),
                _ if !kind.is_empty() => Some(Cue::Entity { kind: kind.to_owned(), event: p.sound_id, at: at(p.position) }),
                _ => Some(Cue::World { event: p.sound_id, at: at(p.position) }),
            }
        }
        LevelEvent::ID => {
            let p: LevelEvent = packet.decode().ok()?;
            // The server sends no break sound: the client plays it with the break particles.
            (p.event == LevelEventEvent::ParticleDestroy).then(|| Some(Cue::Block { block: block_name(bot, p.data)?, event: "break".into(), at: at(p.position) }))?
        }
        _ => None,
    }
}

/// Blocks walked on the ground between footsteps (Java's stride, near enough).
const STRIDE: f32 = 1.6;
/// Ticks between the hit sounds of a block being mined (Java's `destroyTicks % 4`).
const HIT_EVERY: u32 = 4;

/// The sounds the client makes itself, which no server sends: its own footsteps and the knocks
/// of a block it mines.
#[derive(Default)]
pub struct Own {
    last: Option<[f32; 3]>,
    walked: f32,
    mining_ticks: u32,
}

impl Own {
    /// Called once a tick.
    pub fn tick(&mut self, bot: &Bot) -> Vec<Cue> {
        let mut out = Vec::new();
        let Some(m) = bot.movement() else { return out };
        let (Some(feet), on_ground) = (m.position(), m.on_ground()) else { return out };
        if let Some(last) = self.last.replace(feet)
            && on_ground
        {
            self.walked += ((feet[0] - last[0]).powi(2) + (feet[2] - last[2]).powi(2)).sqrt();
            if self.walked >= STRIDE {
                self.walked = 0.0;
                let below = [feet[0].floor() as i32, (feet[1] - 0.2).floor() as i32, feet[2].floor() as i32];
                if let Some(block) = bot.block_name(below).filter(|n| *n != "minecraft:air") {
                    out.push(Cue::Block { block: block.to_owned(), event: "step".into(), at: feet.map(f64::from) });
                }
            }
        }
        match bot.mining_progress() {
            Some((pos, _)) => {
                if self.mining_ticks % HIT_EVERY == 0
                    && let Some(block) = bot.block_name(pos)
                {
                    let at = [0, 1, 2].map(|i| f64::from(pos[i]) + 0.5);
                    out.push(Cue::Block { block: block.to_owned(), event: "hit".into(), at });
                }
                self.mining_ticks += 1;
            }
            None => self.mining_ticks = 0,
        }
        out
    }
}

/// A block broken (the `ParticleDestroy` level event, sent to the breaker too): where, and its
/// runtime id in the shown world, for the chips.
pub fn broken(bot: &Bot, packet: &RawPacket) -> Option<(glam::IVec3, u32)> {
    let p: LevelEvent = packet.decode().ok().filter(|p: &LevelEvent| p.event == LevelEventEvent::ParticleDestroy)?;
    let world = bot.world()?.view()?.world().clone();
    let pos = glam::DVec3::new(p.position.x.into(), p.position.y.into(), p.position.z.into()).floor().as_ivec3();
    Some((pos, world.runtime_id(u32::try_from(p.data).ok()?)))
}

/// The block a sound's data names: the server's id for it (hashed on BDS).
fn block_name(bot: &Bot, wire: i32) -> Option<String> {
    let view = bot.world()?.view()?;
    let world = view.world();
    let id = world.runtime_id(u32::try_from(wire).ok()?);
    Some(world.registry().get(id).filter(|s| !s.is_air())?.name.to_owned())
}

/// Playback for both looks: the Bedrock look plays the pack's own files; the Java look prefers
/// Java's `.ogg` where its files have one at the same path.
/// Built for one look at a time (it holds the audio device), anew when the look changes.
pub struct Audio {
    bedrock: std::path::PathBuf,
    java: std::path::PathBuf,
    sounds: Option<(LookChoice, Sounds)>,
}

impl Audio {
    /// Bedrock's tables and files come from the resource pack itself (`ACACIA_ASSETS`): look packs
    /// don't copy its 150 MB of sounds. Java's `.ogg` files sit in the Java look's `sounds/`.
    pub fn new(looks: &Looks) -> Audio {
        Audio { bedrock: Pack::default_dir(), java: looks.get(LookChoice::Java).files().to_owned(), sounds: None }
    }

    /// `master` 0 to 1 scales every sound.
    pub fn play(&mut self, look: LookChoice, cue: &Cue, camera: &Camera, master: f32) {
        if master <= 0.0 {
            return;
        }
        if self.sounds.as_ref().is_none_or(|(l, _)| *l != look) {
            let roots = match look {
                LookChoice::Bedrock => vec![self.bedrock.clone()],
                LookChoice::Java => vec![self.java.clone(), self.bedrock.clone()],
            };
            self.sounds = Some((look, Sounds::new(&self.bedrock, roots)));
        }
        let Some((_, sounds)) = &mut self.sounds else { return };
        let r = camera.right();
        let listener = Listener { position: camera.position.to_array(), right: [r.x, r.y, r.z] };
        match cue {
            Cue::Named { name, at, volume, pitch } => sounds.play(Event::Named(name), *at, volume * master, *pitch, &listener),
            Cue::Block { block, event, at } => sounds.play(Event::Block { block, event }, Some(*at), master, 1.0, &listener),
            Cue::Entity { kind, event, at } => sounds.play(Event::Entity { kind, event }, Some(*at), master, 1.0, &listener),
            Cue::World { event, at } => sounds.play(Event::World(event), Some(*at), master, 1.0, &listener),
        }
    }
}
