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
    /// A track of this music event (`music.rs`).
    Music(&'static str),
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

/// What the client makes itself, which no server sends: its own footsteps, the knocks of a block
/// it mines, and that block's break (BDS sends the breaker no `ParticleDestroy`: the client
/// predicted it).
#[derive(Default)]
pub struct Own {
    last: Option<[f32; 3]>,
    walked: f32,
    mining_ticks: u32,
    /// The block being mined: where, its name and runtime id.
    mined: Option<(glam::IVec3, &'static str, u32)>,
    music: crate::music::Music,
}

/// A tick's own effects.
#[derive(Default)]
pub struct OwnTick {
    pub sounds: Vec<Cue>,
    /// A block of ours broke: where and its runtime id, for the chips.
    pub broken: Option<(glam::IVec3, u32)>,
}

impl Own {
    /// Called once a tick.
    pub fn tick(&mut self, bot: &Bot) -> OwnTick {
        let mut out = OwnTick { sounds: self.steps(bot), broken: None };
        let player = &bot.state().player;
        if let Some(event) = self.music.tick(player.dimension, player.game_mode == acacia_bot::proto::types::GameMode::Creative) {
            tracing::info!(target: "music", event, "track");
            out.sounds.push(Cue::Music(event));
        }
        match bot.mining_progress() {
            Some((pos, _)) => {
                let p = glam::IVec3::from_array(pos);
                if let Some((block, id)) = block_at(bot, p) {
                    self.mined = Some((p, block, id));
                    if self.mining_ticks.is_multiple_of(HIT_EVERY) {
                        out.sounds.push(Cue::Block { block: block.to_owned(), event: "hit".into(), at: centre(p) });
                    }
                }
                self.mining_ticks += 1;
            }
            None => self.mining_ticks = 0,
        }
        // Mining ends with the prediction; the block turns to air when the server agrees.
        if let Some((p, block, id)) = self.mined
            && bot.mining_progress().is_none_or(|(q, _)| glam::IVec3::from_array(q) != p)
        {
            match block_at(bot, p) {
                None => {
                    out.sounds.push(Cue::Block { block: block.to_owned(), event: "break".into(), at: centre(p) });
                    out.broken = Some((p, id));
                    self.mined = None;
                }
                Some((now, _)) if now != block => self.mined = None,
                Some(_) if bot.mining_progress().is_some() => self.mined = None,
                Some(_) => {}
            }
        }
        out
    }

    fn steps(&mut self, bot: &Bot) -> Vec<Cue> {
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
        out
    }
}

/// The block at `p` and its runtime id; `None` for air or an unknown place.
fn block_at(bot: &Bot, p: glam::IVec3) -> Option<(&'static str, u32)> {
    let view = bot.world()?.view()?;
    let id = acacia_world::BlockAccess::block(view, p.x, p.y, p.z);
    let state = view.world().registry().get(id).filter(|s| !s.is_air())?;
    Some((state.name, id))
}

fn centre(p: glam::IVec3) -> [f64; 3] {
    (p.as_dvec3() + 0.5).to_array()
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
/// Java's `.ogg` where its files have one at the same path. An `audio` thread owns the device and
/// does the loading and decoding (the first sound took 1.3 s, a new sample ~0.3 s, on the frame);
/// it ends when this is dropped.
pub struct Audio {
    plays: std::sync::mpsc::Sender<Play>,
}

struct Play {
    look: LookChoice,
    cue: Cue,
    listener: Listener,
    master: f32,
}

impl Audio {
    /// Bedrock's tables and files come from the resource pack itself (`ACACIA_ASSETS`): look packs
    /// don't copy its 150 MB of sounds. Java's `.ogg` files sit in the Java look's `sounds/`.
    pub fn new(looks: &Looks) -> Audio {
        let (bedrock, java) = (Pack::default_dir(), looks.get(LookChoice::Java).files().to_owned());
        let (plays, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("audio".into())
            .spawn(move || run(&bedrock, &java, &rx))
            .expect("spawn audio thread");
        Audio { plays }
    }

    /// `master` 0 to 1 scales every sound.
    pub fn play(&mut self, look: LookChoice, cue: &Cue, camera: &Camera, master: f32) {
        if master <= 0.0 {
            return;
        }
        let r = camera.right();
        let listener = Listener { position: camera.position.to_array(), right: [r.x, r.y, r.z] };
        let _ = self.plays.send(Play { look, cue: cue.clone(), listener, master });
    }
}

/// Built for one look at a time (it holds the audio device), anew when the look changes.
fn run(bedrock: &std::path::Path, java: &std::path::Path, plays: &std::sync::mpsc::Receiver<Play>) {
    let mut sounds: Option<(LookChoice, Sounds)> = None;
    for Play { look, cue, listener, master } in plays.iter() {
        if sounds.as_ref().is_none_or(|(l, _)| *l != look) {
            let roots = match look {
                // The Bedrock pack has no music files: those come from the Java look's.
                LookChoice::Bedrock => vec![bedrock.to_owned(), java.to_owned()],
                LookChoice::Java => vec![java.to_owned(), bedrock.to_owned()],
            };
            sounds = Some((look, Sounds::new(bedrock, roots)));
        }
        let Some((_, sounds)) = &mut sounds else { continue };
        match &cue {
            Cue::Named { name, at, volume, pitch } => sounds.play(Event::Named(name), *at, volume * master, *pitch, &listener),
            Cue::Block { block, event, at } => sounds.play(Event::Block { block, event }, Some(*at), master, 1.0, &listener),
            Cue::Entity { kind, event, at } => sounds.play(Event::Entity { kind, event }, Some(*at), master, 1.0, &listener),
            Cue::World { event, at } => sounds.play(Event::World(event), Some(*at), master, 1.0, &listener),
            Cue::Music(event) => sounds.stream(event, master),
        }
    }
}
