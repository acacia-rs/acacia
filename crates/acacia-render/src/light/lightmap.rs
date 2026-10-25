//! The lightmap: the colour of every (block, sky) level pair, recomputed each frame on the CPU and
//! sampled by the world shaders (`lightmap` in `gpu/globals.wgsl`), as Java renders `lightmap.fsh`
//! into a 16x16 texture.

/// Texels, `[sky * 16 + block]`, RGBA8 as Java's lightmap texture holds them.
pub type Table = [[u8; 4]; 256];

/// What `lightmap.fsh` reads: Java's `LightmapRenderState`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightmapState {
    /// Sky brightness scale: the time of day, weather, lightning and the End's flashes.
    pub sky_factor: f32,
    pub sky_color: [f32; 3],
    /// Block brightness scale, with the torch flicker.
    pub block_factor: f32,
    pub block_tint: [f32; 3],
    /// What a cell without light shows: the dimension's ambient colour.
    pub ambient: [f32; 3],
    pub night_vision: f32,
    pub night_vision_color: [f32; 3],
    /// Taken off every channel by the darkness effect's pulse.
    pub darkness: f32,
    /// The brightness option, less the darkness effect.
    pub brightness: f32,
}

impl LightmapState {
    pub fn table(&self) -> Table {
        let mut out = [[0; 4]; 256];
        for (i, texel) in out.iter_mut().enumerate() {
            *texel = unorm(self.texel((i % 16) as f32 / 15.0, (i / 16) as f32 / 15.0));
        }
        out
    }

    /// `lightmap.fsh` at block and sky level fractions 0..=1.
    fn texel(&self, block: f32, sky: f32) -> [f32; 3] {
        let curved = |level: f32| level / (4.0 - 3.0 * level);
        let block_brightness = curved(block) * self.block_factor;
        let sky_brightness = curved(sky) * self.sky_factor;
        let vision = self.night_vision_color.map(|c| c * self.night_vision);
        let parabolic = (2.0 * block - 1.0).powi(2);
        let mut color = [0.0f32; 3];
        for i in 0..3 {
            let block_color = lerp(0.9 * parabolic, self.block_tint[i], 1.0);
            color[i] = self.ambient[i].max(vision[i]) + self.sky_color[i] * sky_brightness + block_color * block_brightness;
            color[i] = (color[i] - self.darkness).clamp(0.0, 1.0);
        }
        let max = color[0].max(color[1]).max(color[2]).max(1e-6);
        let not_gamma = (1.0 - (1.0 - max).powi(4)) / max;
        color.map(|c| lerp(self.brightness, c, c * not_gamma))
    }
}

/// The renderer's own curve, kept for the Bedrock look: `max(block, sky - darken)` through Java's
/// curve at half brightness, raised by `ambient`, the same in every channel.
pub fn curve_table(ambient: f32, darken: f32) -> Table {
    let mut out = [[0; 4]; 256];
    for (i, texel) in out.iter_mut().enumerate() {
        let f = ((i % 16) as f32).max((i / 16) as f32 - darken) / 15.0;
        let dark = f / (4.0 - 3.0 * f);
        let b = lerp(0.5, dark, 1.0 - (1.0 - dark).powi(4));
        *texel = unorm([b + ambient * (1.0 - b); 3]);
    }
    out
}

/// What the shaders' `lightmap` returns at fractional levels: Java samples the texture with linear
/// filtering at texel centres, so levels between two blend their texels.
pub fn sample(table: &Table, block: f32, sky: f32) -> [f32; 3] {
    let (b, s) = (block.clamp(0.0, 15.0), sky.clamp(0.0, 15.0));
    let (b0, s0) = (b.floor() as usize, s.floor() as usize);
    let (b1, s1) = ((b0 + 1).min(15), (s0 + 1).min(15));
    let texel = |b: usize, s: usize| table[s * 16 + b].map(|c| f32::from(c) / 255.0);
    let (fb, fs) = (b - b.floor(), s - s.floor());
    let row = |s| {
        let (a, c) = (texel(b0, s), texel(b1, s));
        [0, 1, 2].map(|i| lerp(fb, a[i], c[i]))
    };
    let (low, high) = (row(s0), row(s1));
    [0, 1, 2].map(|i| lerp(fs, low[i], high[i]))
}

/// Four texels per word, as `gpu/globals.wgsl` unpacks them.
pub fn packed(table: &Table) -> [[u32; 4]; 64] {
    let mut out = [[0; 4]; 64];
    for (i, texel) in table.iter().enumerate() {
        out[i / 4][i % 4] = u32::from_le_bytes(*texel);
    }
    out
}

fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + (b - a) * t
}

/// A float written to an 8-bit unorm target: rounded to nearest.
fn unorm(rgb: [f32; 3]) -> [u8; 4] {
    let [r, g, b] = rgb.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
    [r, g, b, 255]
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: LightmapState = LightmapState {
        sky_factor: 1.0,
        sky_color: [1.0; 3],
        block_factor: 1.4,
        block_tint: [1.0, 216.0 / 255.0, 140.0 / 255.0],
        ambient: [0.0; 3],
        night_vision: 0.0,
        night_vision_color: [0.6; 3],
        darkness: 0.0,
        brightness: 0.5,
    };

    #[test]
    fn full_sky_is_white_and_no_light_is_black() {
        let t = DAY.table();
        assert_eq!(t[15 * 16], [255; 4]);
        assert_eq!(t[0], [0, 0, 0, 255]);
        let lamp = t[12];
        assert!(lamp[0] > lamp[1] && lamp[1] > lamp[2], "block light is warm: {lamp:?}");
    }

    #[test]
    fn night_vision_and_ambient_lift_the_dark() {
        let vision = LightmapState { night_vision: 1.0, ..DAY }.table();
        assert!(vision[0][0] > 150, "{:?}", vision[0]);
        let nether = LightmapState { ambient: [0.1; 3], ..DAY }.table();
        assert!(nether[0][0] > 20);
        let dark = LightmapState { darkness: 1.0, ..DAY }.table();
        assert_eq!(dark[15 * 16], [0, 0, 0, 255]);
    }

    #[test]
    fn samples_blend_neighbouring_levels() {
        let t = DAY.table();
        let at = |b: usize, s: usize| t[s * 16 + b].map(|c| f32::from(c) / 255.0);
        assert_eq!(sample(&t, 7.0, 3.0), [at(7, 3)[0], at(7, 3)[1], at(7, 3)[2]]);
        let half = sample(&t, 7.5, 0.0);
        assert!((half[0] - (at(7, 0)[0] + at(8, 0)[0]) / 2.0).abs() < 1e-6);
        assert_eq!(sample(&t, 20.0, -1.0), sample(&t, 15.0, 0.0));
    }

    #[test]
    fn the_bedrock_curve_follows_the_darkened_sky() {
        let t = curve_table(0.0, 11.0);
        assert_eq!(t[15 * 16], t[4]);
        assert_eq!(curve_table(0.0, 0.0)[255], [255; 4]);
        assert_eq!(packed(&t)[0][1], u32::from_le_bytes(t[1]));
    }
}
