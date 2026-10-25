//! The environment attributes the sky, fog, clouds and light read (`EnvironmentAttributes`), and
//! the weather layers over them (`WeatherAttributes`).

use super::timeline::Value4;

macro_rules! attributes {
    ($($attr:ident = $name:literal, $default:expr;)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Attr { $($attr),* }

        impl Attr {
            pub const ALL: &[Attr] = &[$(Attr::$attr),*];

            /// Its id in the game's data, `minecraft:visual/sky_color`.
            pub fn name(self) -> &'static str {
                match self { $(Attr::$attr => $name),* }
            }

            /// Its value where no layer sets it.
            pub fn default(self) -> Value4 {
                match self { $(Attr::$attr => $default),* }
            }
        }
    };
}

const fn rgb(c: u32) -> Value4 {
    [(c >> 16 & 255) as f32 / 255.0, (c >> 8 & 255) as f32 / 255.0, (c & 255) as f32 / 255.0, 1.0]
}

const fn num(v: f32) -> Value4 {
    [v, 0.0, 0.0, 0.0]
}

attributes! {
    SkyColor = "minecraft:visual/sky_color", rgb(0);
    FogColor = "minecraft:visual/fog_color", rgb(0);
    CloudColor = "minecraft:visual/cloud_color", [0.0; 4];
    CloudHeight = "minecraft:visual/cloud_height", num(192.33);
    SunriseSunsetColor = "minecraft:visual/sunrise_sunset_color", [0.0; 4];
    SkyLightFactor = "minecraft:visual/sky_light_factor", num(1.0);
    SkyLightColor = "minecraft:visual/sky_light_color", rgb(0xFFFFFF);
    BlockLightTint = "minecraft:visual/block_light_tint", rgb(0xFFD88C);
    AmbientLightColor = "minecraft:visual/ambient_light_color", rgb(0);
    SunAngle = "minecraft:visual/sun_angle", num(0.0);
    MoonAngle = "minecraft:visual/moon_angle", num(0.0);
    StarAngle = "minecraft:visual/star_angle", num(0.0);
    StarBrightness = "minecraft:visual/star_brightness", num(0.0);
    MoonPhase = "minecraft:visual/moon_phase", num(0.0);
    SkyLightLevel = "minecraft:gameplay/sky_light_level", num(15.0);
    FogStart = "minecraft:visual/fog_start_distance", num(0.0);
    FogEnd = "minecraft:visual/fog_end_distance", num(1024.0);
    SkyFogEnd = "minecraft:visual/sky_fog_end_distance", num(512.0);
    CloudFogEnd = "minecraft:visual/cloud_fog_end_distance", num(2048.0);
    WaterFogColor = "minecraft:visual/water_fog_color", rgb(0x050533);
    WaterFogStart = "minecraft:visual/water_fog_start_distance", num(-8.0);
    WaterFogEnd = "minecraft:visual/water_fog_end_distance", num(96.0);
    NightVisionColor = "minecraft:visual/night_vision_color", rgb(0x999999);
}

impl Attr {
    /// `AttributeRange.sanitize`: the bounds the attribute is clamped to after every layer.
    pub fn range(self) -> Option<(f32, f32)> {
        match self {
            Attr::FogEnd | Attr::SkyFogEnd | Attr::CloudFogEnd | Attr::WaterFogEnd => Some((0.0, f32::INFINITY)),
            Attr::StarBrightness | Attr::SkyLightFactor => Some((0.0, 1.0)),
            Attr::SkyLightLevel => Some((0.0, 15.0)),
            _ => None,
        }
    }

    /// How many of its [`Value4`] lanes it uses: 3 for an RGB colour, 4 for an ARGB one, else 1.
    pub fn arity(self) -> usize {
        match self {
            Attr::SkyColor | Attr::FogColor | Attr::SkyLightColor | Attr::BlockLightTint | Attr::AmbientLightColor | Attr::WaterFogColor | Attr::NightVisionColor => 3,
            Attr::CloudColor | Attr::SunriseSunsetColor => 4,
            _ => 1,
        }
    }

    pub fn by_name(name: &str) -> Option<Attr> {
        Attr::ALL.iter().copied().find(|a| a.name() == name)
    }
}

/// Every attribute's value at one place and time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attributes {
    values: [Value4; Attr::ALL.len()],
}

impl Default for Attributes {
    fn default() -> Self {
        let mut values = [[0.0; 4]; Attr::ALL.len()];
        for (slot, attr) in values.iter_mut().zip(Attr::ALL) {
            *slot = attr.default();
        }
        Attributes { values }
    }
}

impl Attributes {
    pub fn get(&self, attr: Attr) -> Value4 {
        self.values[attr as usize]
    }

    pub fn set(&mut self, attr: Attr, value: Value4) {
        self.values[attr as usize] = value;
    }

    pub fn number(&self, attr: Attr) -> f32 {
        self.get(attr)[0]
    }

    pub fn rgb(&self, attr: Attr) -> [f32; 3] {
        let [r, g, b, _] = self.get(attr);
        [r, g, b]
    }

    fn set_rgb(&mut self, attr: Attr, [r, g, b]: [f32; 3]) {
        let a = self.get(attr)[3];
        self.set(attr, [r, g, b, a]);
    }

    fn set_number(&mut self, attr: Attr, v: f32) {
        self.set(attr, [v, 0.0, 0.0, 0.0]);
    }

    /// `WeatherAttributes.addBuiltinLayers`: the rain layer at what rain thunder does not cover,
    /// then the thunder layer. Only for a dimension with sky light and no ceiling.
    pub fn apply_weather(&mut self, rain: f32, thunder: f32) {
        let grey = |c: [f32; 3]| c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11;
        let lerp = |t: f32, a: f32, b: f32| a + (b - a) * t;
        let lerp3 = |t: f32, a: [f32; 3], b: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| lerp(t, a[i], b[i])) };
        // The layers' colours are 8-bit (`ARGB.colorFromFloat` floors).
        let q = |v: f32| (v * 255.0).floor() / 255.0;
        let night = [q(0.48), q(0.48), 1.0];
        // (level, sky grey brightness and share, fog multiplier, cloud grey brightness and share, light share)
        let layers = [
            (rain - thunder, (0.6, 0.75), [q(0.5), q(0.5), q(0.6)], (0.24, 0.5), 0.3125),
            (thunder, (0.24, 0.94), [q(0.25), q(0.25), q(0.3)], (0.095, 0.94), 0.527_343_75),
        ];
        for (level, (sky_b, sky_f), fog, (cloud_b, cloud_f), share) in layers {
            if level <= 0.0 {
                continue;
            }
            let sky = self.rgb(Attr::SkyColor);
            self.set_rgb(Attr::SkyColor, lerp3(level, sky, lerp3(sky_f, sky, [grey(sky) * sky_b; 3])));
            let fogged = self.rgb(Attr::FogColor);
            self.set_rgb(Attr::FogColor, lerp3(level, fogged, std::array::from_fn(|i| fogged[i] * fog[i])));
            let cloud = self.rgb(Attr::CloudColor);
            self.set_rgb(Attr::CloudColor, lerp3(level, cloud, lerp3(cloud_f, cloud, [grey(cloud) * cloud_b; 3])));
            let level_now = self.number(Attr::SkyLightLevel);
            self.set_number(Attr::SkyLightLevel, lerp(level, level_now, lerp(share, level_now, 4.0)));
            let light = self.rgb(Attr::SkyLightColor);
            self.set_rgb(Attr::SkyLightColor, lerp3(level, light, lerp3(q(share), light, night)));
            let factor = self.number(Attr::SkyLightFactor);
            self.set_number(Attr::SkyLightFactor, lerp(level, factor, lerp(share, factor, 0.24)));
            self.set_number(Attr::StarBrightness, lerp(level, self.number(Attr::StarBrightness), 0.0));
            let s = self.get(Attr::SunriseSunsetColor);
            self.set(Attr::SunriseSunsetColor, std::array::from_fn(|i| if i < 3 { lerp(level, s[i], s[i] * fog[i]) } else { s[3] }));
        }
    }

    /// Clamps each attribute to its [`Attr::range`].
    pub fn sanitize(&mut self) {
        for &attr in Attr::ALL {
            if let Some((low, high)) = attr.range() {
                self.values[attr as usize][0] = self.values[attr as usize][0].clamp(low, high);
            }
        }
    }

    /// `ClientLevel`'s layers while `skyFlashTime` is above 0: a lightning flash pulls the sky 22%
    /// of the way to a pale blue and lights it fully.
    pub fn apply_flash(&mut self) {
        let sky = self.rgb(Attr::SkyColor);
        let flash = [0.8, 0.8, 1.0];
        self.set_rgb(Attr::SkyColor, std::array::from_fn(|i| sky[i] + (flash[i] - sky[i]) * 0.22));
        self.set_number(Attr::SkyLightFactor, 1.0);
    }

    /// Sky light levels the time and weather take off (15 less `gameplay/sky_light_level`): what
    /// the renderer's light curve subtracts.
    pub fn darken(&self) -> f32 {
        (15.0 - self.number(Attr::SkyLightLevel)).max(0.0)
    }

    pub fn moon_phase(&self) -> u8 {
        (self.number(Attr::MoonPhase) as u8).min(7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_defaults_hold() {
        for &a in Attr::ALL {
            assert_eq!(Attr::by_name(a.name()), Some(a));
        }
        let d = Attributes::default();
        assert_eq!((d.number(Attr::FogEnd), d.number(Attr::SkyLightLevel), d.darken()), (1024.0, 15.0, 0.0));
        assert_eq!(d.rgb(Attr::WaterFogColor), [5.0 / 255.0, 5.0 / 255.0, 0x33 as f32 / 255.0]);
    }

    #[test]
    fn a_storm_greys_the_sky_and_takes_the_stars() {
        let mut a = Attributes::default();
        a.set(Attr::SkyColor, rgb(0x78A7FF));
        a.set(Attr::StarBrightness, [0.5, 0.0, 0.0, 0.0]);
        let clear = a;
        a.apply_weather(1.0, 1.0);
        assert_eq!(a.number(Attr::StarBrightness), 0.0);
        let [r, g, b] = a.rgb(Attr::SkyColor);
        assert!((r - g).abs() < 0.05 && (g - b).abs() < 0.05 && b < clear.rgb(Attr::SkyColor)[2] * 0.5, "{:?}", [r, g, b]);
        // Thunder takes the sky light most of the way to level 4.
        assert!((a.number(Attr::SkyLightLevel) - (15.0 + (4.0 - 15.0) * 0.527_343_75)).abs() < 1e-4);
        let mut rain = clear;
        rain.apply_weather(0.0, 0.0);
        assert_eq!(rain, clear);
    }
}
