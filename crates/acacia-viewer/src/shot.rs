//! Unattended capture (`ACACIA_SCREENSHOT=out.png`, `ACACIA_SHOT_AFTER=secs` after the world
//! arrives, `ACACIA_LOOK=yaw,pitch` in degrees, `ACACIA_RISE=blocks` above the bot); the viewer
//! exits after, or at once with an error when the session ends first.

use std::time::{Duration, Instant};

use acacia_render::Camera;
use glam::DVec3;

pub struct Shot {
    pub path: std::path::PathBuf,
    pub after: Duration,
    pub world_at: Option<Instant>,
    pub taken: bool,
}

impl Shot {
    pub fn from_env() -> Option<Shot> {
        let path = std::env::var_os("ACACIA_SCREENSHOT")?.into();
        let after = std::env::var("ACACIA_SHOT_AFTER").ok().and_then(|s| s.parse().ok()).unwrap_or(15.0);
        Some(Shot { path, after: Duration::from_secs_f32(after), world_at: None, taken: false })
    }

    pub fn place(camera: &mut Camera, player: DVec3) {
        let rise: f64 = std::env::var("ACACIA_RISE").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        camera.position = player + DVec3::Y * rise;
        if let Some((yaw, pitch)) = std::env::var("ACACIA_LOOK").ok().and_then(|s| {
            let (y, p) = s.split_once(',')?;
            Some((y.parse::<f32>().ok()?, p.parse::<f32>().ok()?))
        }) {
            (camera.yaw, camera.pitch) = (yaw.to_radians(), pitch.to_radians());
        }
    }
}
