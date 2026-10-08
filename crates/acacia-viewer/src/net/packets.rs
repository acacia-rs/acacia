//! Packets the bot thread passes to the window as they arrive: sounds, broken blocks, particles
//! and biome colours.

use std::path::Path;
use std::sync::Arc;

use acacia_bot::Bot;
use acacia_bot::proto::packets::{BiomeDefinitionList, SpawnParticleEffect};
use acacia_bot::proto::{DecodeError, Packet, RawPacket};
use acacia_render::biome::{BiomeColors, BiomeDef};

use super::NetEvent;
use crate::{audio, particles};

/// What the bot subscribes to for [`events`].
pub fn forwarded() -> impl Iterator<Item = u32> {
    audio::PACKETS.into_iter().chain([SpawnParticleEffect::ID, BiomeDefinitionList::ID])
}

/// The window's events from one packet; `files` is the look pack's.
pub fn events(bot: &Bot, packet: &RawPacket, files: &Path) -> Result<Vec<NetEvent>, DecodeError> {
    if packet.id == BiomeDefinitionList::ID {
        let defs = biome_defs(&packet.decode()?);
        tracing::info!(count = defs.len(), "biome definitions");
        return Ok(vec![NetEvent::Biomes(Arc::new(BiomeColors::build(&defs, files)))]);
    }
    let mut out: Vec<NetEvent> = particles::spawns(packet).into_iter().map(NetEvent::Particles).collect();
    out.extend(audio::cue(bot, packet).map(NetEvent::Sound));
    out.extend(audio::broken(bot, packet).map(|(pos, block)| NetEvent::Broken { pos, block }));
    Ok(out)
}

fn biome_defs(list: &BiomeDefinitionList) -> Vec<BiomeDef> {
    list.biome_definitions
        .iter()
        .filter_map(|d| {
            let name = list.string_list.get(usize::try_from(d.name_index).ok()?)?;
            let name = name.strip_prefix("minecraft:").unwrap_or(name).to_owned();
            Some(BiomeDef { id: d.biome_id, name, temperature: d.temperature, downfall: d.downfall })
        })
        .collect()
}
