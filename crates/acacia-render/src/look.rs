//! What differs between the editions' looks at draw time. Design: docs/java-look.md.

use serde::{Deserialize, Serialize};

/// How distance fog closes in on the far edge of the loaded terrain.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fog {
    /// Measured from the vertical axis through the camera, not from the camera.
    pub cylinder: bool,
    /// A linear ramp, not a smoothstep.
    pub linear: bool,
    /// Share of the fog distance the ramp takes, before `band_limits`.
    pub band: f32,
    /// Shortest and longest ramp in blocks.
    pub band_limits: (f32, f32),
    /// A second fog over the first: linear over these distances from the camera, whatever the
    /// first one's shape.
    pub haze: Option<Haze>,
}

/// Where a haze starts and where it is opaque, in blocks.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Haze {
    pub overworld: (f32, f32),
    pub nether: (f32, f32),
}

impl Fog {
    /// Distance where fog begins when it is opaque at `end`.
    pub fn start(&self, end: f32) -> f32 {
        end - (end * self.band).clamp(self.band_limits.0, self.band_limits.1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Look {
    pub fog: Fog,
    /// Water surface opacity; `None` keeps the texture's alpha.
    pub water_alpha: Option<f32>,
    /// Columns to each side whose biome colours a tint averages; baked into the block table, so
    /// changing it takes a remesh.
    pub biome_blend: u8,
    /// Brightness of a face corner by how open it is: from three darkening blocks around it
    /// (or one on each side) to none.
    pub ambient_occlusion: [f32; 4],
    /// Packs baked before this field have none: they take Java's.
    #[serde(default)]
    pub dropped: Dropped,
    /// Packs baked before this field have none: they take the pack's.
    #[serde(default)]
    pub fluid_fog: FluidFog,
}

/// Whose rules set the fog with the camera in water, lava or powder snow ([`crate::fluid_view`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FluidFog {
    /// The look's fog files, closing in as their `transition_fog` says.
    #[default]
    Pack,
    /// Java's `FogRenderer`: water fog reaching further and brightening with the time underwater
    /// (`getWaterVision`); lava and powder snow fogs from its code.
    Java,
}

/// How a dropped item hovers, turns and shows its stack size. Bedrock's are unmeasured, so both
/// looks use Java's (`ItemEntityRenderer`, `ItemClusterRenderState`, the item models' `ground`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Dropped {
    /// The lowest point floats this far over the feet, plus the bob.
    pub hover: f32,
    /// The bob is `sin(age / bob_ticks + offset) · bob + bob`, in blocks.
    pub bob: f32,
    pub bob_ticks: f32,
    /// Ticks per radian of the turn.
    pub spin_ticks: f32,
    /// Copies drawn: one more past each of these stack sizes.
    pub copies: [u16; 4],
    /// Furthest a copy strays from the first, in blocks.
    pub scatter: f32,
    /// Scale and lift of a flat item and of a block.
    pub flat: Ground,
    pub block: Ground,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ground {
    pub scale: f32,
    pub lift: f32,
}

impl Dropped {
    pub const JAVA: Dropped = Dropped {
        hover: 0.0625,
        bob: 0.1,
        bob_ticks: 10.0,
        spin_ticks: 20.0,
        copies: [1, 16, 32, 48],
        scatter: 0.15,
        flat: Ground { scale: 0.5, lift: 2.0 / 16.0 },
        block: Ground { scale: 0.25, lift: 3.0 / 16.0 },
    };
}

impl Default for Dropped {
    fn default() -> Self {
        Dropped::JAVA
    }
}

impl Look {
    /// Water opacity is `biomes_client.json`'s default water_surface_transparency.
    pub const BEDROCK: Look = Look {
        // Not infinity: JSON has none, and a look pack stores this.
        fog: Fog { cylinder: false, linear: false, band: 0.3, band_limits: (0.0, f32::MAX), haze: None },
        water_alpha: Some(0.65),
        biome_blend: 1,
        ambient_occlusion: [0.45, 0.62, 0.8, 1.0],
        dropped: Dropped::JAVA,
        fluid_fog: FluidFog::Pack,
    };
    /// The blend is Java's default `biomeBlendRadius`; the haze its environmental fog
    /// (`fog_start_distance` and `fog_end_distance`: the defaults, and the Nether's), without
    /// what rain and boss fights do to it.
    pub const JAVA: Look = Look {
        fog: Fog {
            cylinder: true,
            linear: true,
            band: 0.1,
            band_limits: (4.0, 64.0),
            haze: Some(Haze { overworld: (0.0, 1024.0), nether: (10.0, 96.0) }),
        },
        water_alpha: None,
        biome_blend: 2,
        // BlockModelLighter: the mean of four cells, each 0.2 when it darkens and 1 when not.
        ambient_occlusion: [0.4, 0.6, 0.8, 1.0],
        dropped: Dropped::JAVA,
        fluid_fog: FluidFog::Java,
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
