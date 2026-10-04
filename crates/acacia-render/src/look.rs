//! What differs between the editions' looks at draw time. Design: docs/java-look.md.

/// How distance fog closes in on the far edge of the loaded terrain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fog {
    /// Measured from the vertical axis through the camera, not from the camera.
    pub cylinder: bool,
    /// A linear ramp, not a smoothstep.
    pub linear: bool,
    /// Share of the fog distance the ramp takes, before `band_limits`.
    pub band: f32,
    /// Shortest and longest ramp in blocks.
    pub band_limits: (f32, f32),
}

impl Fog {
    /// Distance where fog begins when it is opaque at `end`.
    pub fn start(&self, end: f32) -> f32 {
        end - (end * self.band).clamp(self.band_limits.0, self.band_limits.1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub fog: Fog,
    /// Water surface opacity; `None` keeps the texture's alpha.
    pub water_alpha: Option<f32>,
}

impl Look {
    /// Water opacity is `biomes_client.json`'s default water_surface_transparency.
    pub const BEDROCK: Look = Look {
        fog: Fog { cylinder: false, linear: false, band: 0.3, band_limits: (0.0, f32::INFINITY) },
        water_alpha: Some(0.65),
    };
    // TODO: Java's second fog band (the spherical biome haze) is not drawn.
    pub const JAVA: Look = Look {
        fog: Fog { cylinder: true, linear: true, band: 0.1, band_limits: (4.0, 64.0) },
        water_alpha: None,
    };
}

impl Default for Look {
    fn default() -> Self {
        Look::BEDROCK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bedrock_fog_starts_at_seven_tenths() {
        assert!((Look::BEDROCK.fog.start(160.0) - 112.0).abs() < 1e-3);
    }

    #[test]
    fn java_fog_band_is_a_tenth_within_limits() {
        let fog = Look::JAVA.fog;
        assert_eq!(fog.start(128.0), 128.0 - 12.8);
        assert_eq!(fog.start(16.0), 12.0);
        assert_eq!(fog.start(1024.0), 960.0);
    }
}
