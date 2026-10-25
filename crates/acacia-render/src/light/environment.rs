//! The frame's lightmap from what the player sees and feels: the dimension, the time of day, the
//! weather, the brightness option and the effects on them.

use super::lightmap::{LightmapState, Table, curve_table};
use crate::look::Lighting;
use crate::sky::{Realm, Sky, Weather};

/// What the player brings to the lightmap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightConditions {
    /// The brightness option, 0 (moody) to 1 (bright).
    pub gamma: f32,
    /// Night vision strength, 0 to 1 (Java's `GameRenderer.getNightVisionScale`).
    pub night_vision: f32,
    /// The darkness effect: (how far it has faded in, scaled by the option; its pulse).
    pub darkness: (f32, f32),
    /// The torch flicker (Java's `LightmapRenderStateExtractor` random walk).
    pub flicker: f32,
}

impl Default for LightConditions {
    fn default() -> Self {
        LightConditions { gamma: 0.5, night_vision: 0.0, darkness: (0.0, 0.0), flicker: 0.0 }
    }
}

/// Brightness of unlit blocks under the renderer's own curve, with and without sky light.
const CURVE_AMBIENT: (f32, f32) = (0.02, 0.1);
const BLOCK_TINT: [f32; 3] = [1.0, 216.0 / 255.0, 140.0 / 255.0];
const NIGHT_VISION_COLOR: [f32; 3] = [0.6; 3];

pub fn frame_lightmap(lighting: Lighting, realm: Realm, time: f32, weather: Weather, c: &LightConditions) -> Table {
    match lighting {
        Lighting::Volume => {
            let ambient = if realm == Realm::Overworld { CURVE_AMBIENT.0 } else { CURVE_AMBIENT.1 };
            curve_table(ambient, Sky::of(realm, time, weather).darken)
        }
        Lighting::Java => java_state(realm, time, weather, c).table(),
    }
}

/// Java's `LightmapRenderStateExtractor`.
// TODO: the sky factor and colour from 26.3's `day` timeline and weather attributes, and the
// dimensions' ambient colours, once the oracle shows them; the darkening below stands in.
pub fn java_state(realm: Realm, time: f32, weather: Weather, c: &LightConditions) -> LightmapState {
    let darken = Sky::of(realm, time, weather).darken / 11.0;
    let sky_factor = if weather.flash > 0.0 { 1.0 } else { 1.0 - 0.8 * darken };
    let ambient = if realm == Realm::Overworld { [0.0; 3] } else { [0.1; 3] };
    LightmapState {
        sky_factor,
        sky_color: [1.0; 3],
        block_factor: 1.4 + c.flicker,
        block_tint: BLOCK_TINT,
        ambient,
        night_vision: c.night_vision,
        night_vision_color: NIGHT_VISION_COLOR,
        darkness: c.darkness.1,
        brightness: (c.gamma - c.darkness.0).max(0.0),
    }
}
