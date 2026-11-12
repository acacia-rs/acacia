//! Biomes where the client reads them: `BiomeManager.getBiome`'s zoom onto the chunks' 4×4×4 biome cells, and each
//! registry biome's climate.

use acacia_java_proto::tag::Tag;
use acacia_registry::climate::Climate;

/// A `worldgen/biome` entry's climate: the server's data when it sent some, else vanilla's (`None` for a biome
/// known to neither).
pub(crate) fn climate_of(name: &str, data: Option<&Tag>) -> Option<Climate> {
    let Some(data) = data else { return Climate::vanilla(name) };
    Some(Climate {
        temperature: data.get("temperature")?.as_f64()? as f32,
        frozen: data.get("temperature_modifier").and_then(Tag::as_str) == Some("frozen"),
        has_precipitation: data.get("has_precipitation")?.as_bool()?,
    })
}

/// `LinearCongruentialGenerator.next`.
fn lcg(rval: i64, c: i64) -> i64 {
    rval.wrapping_mul(rval.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407)).wrapping_add(c)
}

fn fiddle(rval: i64) -> f64 {
    ((rval >> 24).rem_euclid(1024) as f64 / 1024.0 - 0.5) * 0.9
}

/// `getFiddledDistance`.
fn fiddled_distance(seed: i64, corner: [i32; 3], distance: [f64; 3]) -> f64 {
    let mut rval = seed;
    for _ in 0..2 {
        for c in corner {
            rval = lcg(rval, i64::from(c));
        }
    }
    let fx = fiddle(rval);
    rval = lcg(rval, seed);
    let fy = fiddle(rval);
    rval = lcg(rval, seed);
    let fz = fiddle(rval);
    let square = |v: f64| v * v;
    square(distance[2] + fz) + square(distance[1] + fy) + square(distance[0] + fx)
}

/// `BiomeManager.getBiome`: the biome cell (quart position) a block reads, `zoom_seed` being the hashed seed `Login`
/// and `Respawn` carry.
pub(crate) fn noise_cell(zoom_seed: i64, pos: [i32; 3]) -> [i32; 3] {
    let abs = pos.map(|v| v - 2);
    let parent = abs.map(|v| v >> 2);
    let fract = abs.map(|v| f64::from(v & 3) / 4.0);
    let mut best = (0, f64::INFINITY);
    for i in 0..8 {
        let odd = [i & 4 != 0, i & 2 != 0, i & 1 != 0];
        let corner = [0, 1, 2].map(|a| parent[a] + i32::from(odd[a]));
        let distance = [0, 1, 2].map(|a| if odd[a] { fract[a] - 1.0 } else { fract[a] });
        let next = fiddled_distance(zoom_seed, corner, distance);
        if best.1 > next {
            best = (i, next);
        }
    }
    [0, 1, 2].map(|a| parent[a] + i32::from(best.0 & (4 >> a) != 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_reads_one_of_the_eight_cells_around_it() {
        for pos in [[0, 64, 0], [-7, -60, 13], [1000, 100, -999]] {
            let cell = noise_cell(-4_172_144_997_902_289_642, pos);
            let parent = pos.map(|v| (v - 2) >> 2);
            assert!((0..3).all(|a| cell[a] == parent[a] || cell[a] == parent[a] + 1), "{pos:?} → {cell:?}");
        }
    }

    #[test]
    fn sent_climates_win_over_vanilla() {
        let data = Tag::Compound(vec![("temperature".into(), Tag::Float(2.0)), ("has_precipitation".into(), Tag::Byte(0))]);
        assert_eq!(climate_of("minecraft:plains", Some(&data)).map(|c| c.has_precipitation), Some(false));
        assert_eq!(climate_of("minecraft:plains", None).map(|c| c.has_precipitation), Some(true));
        assert_eq!(climate_of("my:biome", None), None);
    }
}
