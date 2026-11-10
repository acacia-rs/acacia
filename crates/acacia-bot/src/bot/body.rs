//! The moving player as either edition simulates it: what processes and pathing read and steer.

use acacia_physics::Vec3;

use super::bedrock::Backend;
use crate::movement::Controls;

/// The simulated body as a snapshot shows it; velocity in blocks per tick.
pub(crate) struct Motion {
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub gliding: bool,
}

impl Backend {
    /// The simulated body once movement has started; `None` for Bedrock bots without physics.
    pub(crate) fn motion(&self) -> Option<Motion> {
        match self {
            Backend::Bedrock(b) => {
                let m = b.movement.as_ref()?;
                let (yaw, pitch) = (m.controls.yaw, m.controls.pitch);
                Some(Motion { position: m.position()?, velocity: m.velocity().unwrap_or_default(), yaw, pitch, on_ground: m.on_ground(), gliding: m.gliding() })
            }
            #[cfg(feature = "java")]
            Backend::Java(j) => {
                let p = &j.client.movement.player;
                let (position, velocity) = (p.pos.map(|c| c as f32), p.delta.map(|c| c as f32));
                j.started().then_some(Motion { position, velocity, yaw: p.yaw, pitch: p.pitch, on_ground: p.on_ground, gliding: p.fall_flying })
            }
        }
    }

    /// The bot simulates its own movement (a physics bot on Bedrock; always on Java).
    pub(crate) fn moves(&self) -> bool {
        match self {
            Backend::Bedrock(b) => b.movement.is_some(),
            #[cfg(feature = "java")]
            Backend::Java(_) => true,
        }
    }

    /// Feet and on-ground flag once movement has started.
    pub(crate) fn feet(&self) -> Option<(Vec3, bool)> {
        match self {
            Backend::Bedrock(b) => b.movement.as_ref().and_then(|m| Some((m.state()?.pos, m.on_ground()))),
            #[cfg(feature = "java")]
            Backend::Java(j) => j.started().then(|| (j.client.movement.player.pos.map(|c| c as f32), j.client.movement.player.on_ground)),
        }
    }

    pub(crate) fn eye(&self) -> Option<Vec3> {
        match self {
            Backend::Bedrock(b) => b.movement.as_ref()?.eye_position(),
            #[cfg(feature = "java")]
            Backend::Java(j) => {
                let p = &j.client.movement.player;
                j.started().then(|| [p.pos[0] as f32, (p.pos[1] + f64::from(p.pose().eye_height())) as f32, p.pos[2] as f32])
            }
        }
    }

    pub(crate) fn controls(&self) -> Option<&Controls> {
        match self {
            Backend::Bedrock(b) => b.movement.as_ref().map(|m| &m.controls),
            #[cfg(feature = "java")]
            Backend::Java(j) => Some(&j.controls),
        }
    }

    pub(crate) fn controls_mut(&mut self) -> Option<&mut Controls> {
        match self {
            Backend::Bedrock(b) => b.movement.as_mut().map(|m| &mut m.controls),
            #[cfg(feature = "java")]
            Backend::Java(j) => Some(&mut j.controls),
        }
    }
}
