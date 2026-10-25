//! The oracle's scenes (tools/parity/harnesses/weather/Scene.java) as a weather level.

use std::collections::HashMap;

use acacia_render::weather::climate::{Climate, Precipitation, SEA_LEVEL};
use acacia_render::weather::columns::WeatherLevel;
use acacia_render::weather::splash::SplashLevel;
use serde_json::Value;

const MIN_Y: i32 = -64;

/// The top block's collision boxes `[min x, min y, min z, max x, max y, max z]`, fluid height, smoke.
struct Top {
    boxes: Vec<[f64; 6]>,
    fluid: f64,
    smokes: bool,
}

pub struct Scene<'a> {
    x0: i32,
    z0: i32,
    side: i32,
    surface: Vec<i32>,
    biome: Vec<&'a str>,
    loaded: Vec<bool>,
    light: Vec<[u8; 2]>,
    tops: Vec<Top>,
    climates: &'a HashMap<String, Climate>,
}

impl<'a> Scene<'a> {
    pub fn new(case: &'a Value, climates: &'a HashMap<String, Climate>) -> Scene<'a> {
        let ints = |key: &str| case[key].as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i32).collect::<Vec<_>>();
        let origin = ints("origin");
        let tops = case["tops"].as_array().map_or_else(Vec::new, |tops| {
            tops.iter()
                .map(|t| Top {
                    boxes: t["boxes"].as_array().unwrap().iter().map(|b| std::array::from_fn(|i| b[i].as_f64().unwrap())).collect(),
                    // FluidState.getHeight is a float, widened by the game.
                    fluid: f64::from(t["fluid"].as_f64().unwrap() as f32),
                    smokes: t["smokes"].as_bool().unwrap(),
                })
                .collect()
        });
        Scene {
            x0: origin[0],
            z0: origin[1],
            side: case["side"].as_i64().unwrap() as i32,
            surface: ints("surface"),
            biome: case["biome"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect(),
            loaded: case["loaded"].as_array().unwrap().iter().map(|v| v.as_bool().unwrap()).collect(),
            light: case["light"].as_array().unwrap().iter().map(|l| [l[0].as_u64().unwrap() as u8, l[1].as_u64().unwrap() as u8]).collect(),
            tops,
            climates,
        }
    }

    fn index(&self, x: i32, z: i32) -> Option<usize> {
        let (i, k) = (x - self.x0, z - self.z0);
        ((0..self.side).contains(&i) && (0..self.side).contains(&k)).then(|| (k * self.side + i) as usize)
    }

    fn loaded(&self, x: i32, z: i32) -> Option<usize> {
        self.index(x, z).filter(|&i| self.loaded[i])
    }
}

impl WeatherLevel for Scene<'_> {
    fn surface(&self, x: i32, z: i32) -> i32 {
        self.loaded(x, z).map_or(MIN_Y, |i| self.surface[i])
    }

    fn precipitation(&self, [x, y, z]: [i32; 3]) -> Precipitation {
        self.loaded(x, z).map_or(Precipitation::None, |i| self.climates[self.biome[i]].precipitation_at([x, y, z], SEA_LEVEL))
    }

    fn light(&self, [x, _, z]: [i32; 3]) -> [u8; 2] {
        self.index(x, z).map_or([0, 0], |i| self.light[i])
    }
}

impl SplashLevel for Scene<'_> {
    fn min_y(&self) -> i32 {
        MIN_Y
    }

    fn splash_top(&self, [x, _, z]: [i32; 3], bx: f64, bz: f64) -> f64 {
        let top = &self.tops[self.index(x, z).unwrap()];
        let over = |b: &&[f64; 6]| (b[0]..b[3]).contains(&bx) && (b[2]..b[5]).contains(&bz);
        top.boxes.iter().filter(over).map(|b| b[4]).fold(0.0, f64::max).max(top.fluid)
    }

    fn smokes(&self, [x, _, z]: [i32; 3]) -> bool {
        self.tops[self.index(x, z).unwrap()].smokes
    }
}
