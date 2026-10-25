//! Rain and snow columns around the camera and their quads: 26.3's
//! `WeatherEffectRenderer.extractRenderState`, `create{Rain,Snow}ColumnInstance` and
//! `prepareInstances`.

use std::sync::LazyLock;

use glam::DVec3;

use super::climate::Precipitation;
use crate::java_random::JavaRandom;

/// What the weather reads of the world.
pub trait WeatherLevel {
    /// `getHeight(MOTION_BLOCKING, x, z)`: one above the highest block that blocks motion or holds
    /// a fluid.
    fn surface(&self, x: i32, z: i32) -> i32;
    /// `ClientLevel.getPrecipitationAt`: none outside loaded chunks.
    fn precipitation(&self, p: [i32; 3]) -> Precipitation;
    /// `[block, sky]` light levels (`LightCoordsUtil.getLightCoords`).
    fn light(&self, p: [i32; 3]) -> [u8; 2];
}

/// `ColumnInstance`; `light` is `[block, sky]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Column {
    pub x: i32,
    pub z: i32,
    pub bottom: i32,
    pub top: i32,
    pub u_offset: f32,
    pub v_offset: f32,
    pub light: [u8; 2],
}

/// `WeatherRenderState`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Columns {
    pub rain: Vec<Column>,
    pub snow: Vec<Column>,
    pub radius: i32,
    /// `getRainLevel`.
    pub intensity: f32,
}

/// The Weather Radius option's range (`Options.weatherRadius`: 3..=10, 10 by default).
pub const RADIUS: std::ops::RangeInclusive<i32> = 3..=10;

pub fn column_seed(x: i32, z: i32) -> i32 {
    (x.wrapping_mul(x).wrapping_mul(3121).wrapping_add(x.wrapping_mul(45_238_971))) ^ (z.wrapping_mul(z).wrapping_mul(418_711).wrapping_add(z.wrapping_mul(13761)))
}

/// `extractRenderState`: `game_time` is the level's, `radius` the Weather Radius option.
pub fn extract(level: &impl WeatherLevel, intensity: f32, game_time: i64, partial: f32, camera: DVec3, radius: i32) -> Columns {
    let radius = radius.clamp(*RADIUS.start(), *RADIUS.end());
    let mut out = Columns { radius, intensity, ..Default::default() };
    if intensity <= 0.0 {
        return out;
    }
    let [cx, cy, cz] = camera.floor().as_ivec3().to_array();
    for z in cz - radius..=cz + radius {
        for x in cx - radius..=cx + radius {
            let terrain = level.surface(x, z);
            let (y0, y1) = ((cy - radius).max(terrain), (cy + radius).max(terrain));
            if y1 == y0 {
                continue;
            }
            let precipitation = level.precipitation([x, cy, z]);
            if precipitation == Precipitation::None {
                continue;
            }
            let mut random = JavaRandom::new(i64::from(column_seed(x, z)));
            let light = level.light([x, cy.max(terrain), z]);
            let column = Column { x, z, bottom: y0, top: y1, u_offset: 0.0, v_offset: 0.0, light };
            match precipitation {
                Precipitation::Rain => out.rain.push(rain(&mut random, game_time, partial, column)),
                _ => out.snow.push(snow(&mut random, game_time, partial, column)),
            }
        }
    }
    out
}

fn rain(random: &mut JavaRandom, ticks: i64, partial: f32, column: Column) -> Column {
    let (x, z) = (column.x, column.z);
    let wrapped = (ticks & 131_071) as i32;
    let tick_offset = x.wrapping_mul(x).wrapping_mul(3121).wrapping_add(x.wrapping_mul(45_238_971)).wrapping_add(z.wrapping_mul(z).wrapping_mul(418_711)).wrapping_add(z.wrapping_mul(13761)) & 0xFF;
    let speed = 3.0 + random.next_float();
    let offset = -(wrapped.wrapping_add(tick_offset) as f32 + partial) / 32.0 * speed;
    Column { v_offset: offset % 32.0, ..column }
}

fn snow(random: &mut JavaRandom, ticks: i64, partial: f32, column: Column) -> Column {
    let time = (ticks & 131_071) as i32 as f32 + partial;
    let u = (random.next_double() + f64::from(time * 0.01 * random.next_gaussian() as f32)) as f32;
    let v = (random.next_double() + f64::from(time * random.next_gaussian() as f32 * 0.001)) as f32;
    let fall = -((ticks & 511) as f32 + partial) / 512.0;
    let light = column.light.map(|l| (l * 3 + 15) / 4);
    Column { u_offset: u, v_offset: fall + v, light, ..column }
}

/// One vertex of a column quad, camera-relative; `uv` runs past 0..1 for the sampler to repeat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    /// `ARGB.white(alpha)`'s alpha.
    pub alpha: u8,
    pub light: [u8; 2],
}

/// The constructor's table of column half-widths around the camera's column (which divides 0 by 0).
static COLUMN_SIZE: LazyLock<[[f32; 2]; 1024]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let (dx, dz) = ((i % 32) as f32 - 16.0, (i / 32) as f32 - 16.0);
        let distance = f64::from(dx * dx + dz * dz).sqrt() as f32;
        [-dz / distance, dx / distance]
    })
});

/// `prepare`: every rain quad, then every snow one (drawn with their own textures).
pub fn quads(columns: &Columns, camera: DVec3) -> Vec<[WeatherVertex; 4]> {
    let rain = columns.rain.iter().map(|c| quad(c, camera, 1.0, columns));
    rain.chain(columns.snow.iter().map(|c| quad(c, camera, 0.8, columns))).collect()
}

fn quad(column: &Column, camera: DVec3, max_alpha: f32, columns: &Columns) -> [WeatherVertex; 4] {
    let radius_sq = (columns.radius * columns.radius) as f32;
    let rel_x = (f64::from(column.x) + 0.5 - camera.x) as f32;
    let rel_z = (f64::from(column.z) + 0.5 - camera.z) as f32;
    let t = ((rel_x * rel_x + rel_z * rel_z) / radius_sq).min(1.0);
    let alpha = ((max_alpha + t * (0.5 - max_alpha)) * columns.intensity * 255.0).floor() as u8;
    let index = (column.z - camera.z.floor() as i32 + 16) * 32 + column.x - camera.x.floor() as i32 + 16;
    let [size_x, size_z] = COLUMN_SIZE[index as usize];
    let (half_x, half_z) = (size_x / 2.0, size_z / 2.0);
    let (x0, x1, z0, z1) = (rel_x - half_x, rel_x + half_x, rel_z - half_z, rel_z + half_z);
    let y1 = (f64::from(column.top) - camera.y) as f32;
    let y0 = (f64::from(column.bottom) - camera.y) as f32;
    let (u0, u1) = (column.u_offset, column.u_offset + 1.0);
    let v0 = column.bottom as f32 * 0.25 + column.v_offset;
    let v1 = column.top as f32 * 0.25 + column.v_offset;
    let vertex = |pos, uv| WeatherVertex { pos, uv, alpha, light: column.light };
    [vertex([x0, y1, z0], [u0, v0]), vertex([x1, y1, z1], [u1, v0]), vertex([x1, y0, z1], [u1, v1]), vertex([x0, y0, z0], [u0, v1])]
}

#[cfg(test)]
#[path = "columns_tests.rs"]
mod tests;
