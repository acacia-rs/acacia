//! How biome attributes blend around the camera: `EnvironmentAttributeProbe` weighs the noise
//! biomes (4-block cells) with `GaussianSampler`, and `SpatialAttributeInterpolator` lerps the
//! values in the order the biomes were first met.

use super::timeline::Value4;

/// `GaussianSampler.GAUSSIAN_SAMPLE_KERNEL`.
const KERNEL: [f64; 7] = [0.0, 1.0, 4.0, 6.0, 4.0, 1.0, 0.0];

/// Each biome around `position` (blocks) once, with its summed weight, in the order first
/// sampled; zero weights are kept. `biome_at` takes a cell's coordinates (block / 4).
pub fn gaussian_weights<B: PartialEq + Copy>(position: [f64; 3], mut biome_at: impl FnMut(i32, i32, i32) -> B) -> Vec<(B, f64)> {
    let p = position.map(|c| c * 0.25 - 0.5);
    let cell = p.map(|c| c.floor() as i32);
    let relative: [f64; 3] = std::array::from_fn(|i| p[i] - f64::from(cell[i]));
    let weight = |axis: usize, i: usize| KERNEL[i + 1] + relative[axis] * (KERNEL[i] - KERNEL[i + 1]);
    let mut weights: Vec<(B, f64)> = Vec::new();
    for z in 0..6 {
        for x in 0..6 {
            for y in 0..6 {
                let biome = biome_at(cell[0] - 2 + x as i32, cell[1] - 2 + y as i32, cell[2] - 2 + z as i32);
                let w = weight(0, x) * weight(1, y) * weight(2, z);
                match weights.iter_mut().find(|(b, _)| *b == biome) {
                    Some((_, total)) => *total += w,
                    None => weights.push((biome, w)),
                }
            }
        }
    }
    weights
}

/// The weighted mean as the interpolator builds it: each value lerped in by its share so far.
pub fn blend(values: impl IntoIterator<Item = (Value4, f64)>) -> Value4 {
    let mut result: Option<Value4> = None;
    let mut accumulated = 0.0;
    for (v, weight) in values {
        accumulated += weight;
        result = Some(match result {
            None => v,
            Some(r) => {
                let t = (weight / accumulated) as f32;
                std::array::from_fn(|i| r[i] + (v[i] - r[i]) * t)
            }
        });
    }
    result.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_biome_keeps_its_value_and_the_kernel_sums_to_sixteen_cubed() {
        let weights = gaussian_weights([10.3, 64.0, -7.9], |_, _, _| 4u32);
        assert_eq!(weights.len(), 1);
        assert!((weights[0].1 - 16f64.powi(3)).abs() < 1e-9);
        assert_eq!(blend([([0.2, 0.4, 0.6, 1.0], 3.0)]), [0.2, 0.4, 0.6, 1.0]);
    }

    #[test]
    fn a_border_blends_by_distance() {
        // Biome 1 east of x = 0, biome 0 west: at x = 0 both weigh the same.
        let weights = gaussian_weights([0.0, 64.0, 0.0], |x, _, _| u32::from(x >= 0));
        let share = |b: u32| weights.iter().find(|w| w.0 == b).unwrap().1;
        assert!((share(0) - share(1)).abs() < 1e-9, "{weights:?}");
        let mixed = blend(weights.iter().map(|&(b, w)| ([b as f32, 0.0, 0.0, 0.0], w)));
        assert!((mixed[0] - 0.5).abs() < 1e-6);
    }
}
