//! The moving player as either edition simulates it: what processes and pathing read and steer.

use acacia_physics::Vec3;

use super::bedrock::Backend;
use crate::movement::Controls;

impl Backend {
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
