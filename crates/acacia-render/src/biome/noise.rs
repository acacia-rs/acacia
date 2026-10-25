//! Java's 2D `SimplexNoise` without offsets, as `Biome` seeds it: `BIOME_INFO_NOISE` (swamp grass
//! patches, frozen ground), `TEMPERATURE_NOISE` and `FROZEN_TEMPERATURE_NOISE`.

use std::sync::LazyLock;

use crate::blocks::placed::Draw;

/// x and y of `GradientNoise.GRADIENT`'s first twelve.
const GRADIENT: [[f64; 2]; 12] =
    [[1.0, 1.0], [-1.0, 1.0], [1.0, -1.0], [-1.0, -1.0], [1.0, 0.0], [-1.0, 0.0], [1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0], [0.0, 1.0], [0.0, -1.0]];

pub struct Simplex {
    permutation: [u8; 256],
}

impl Simplex {
    /// The next noise drawn from `draw` (`new SimplexNoise(random, true)`).
    pub fn from_draw(draw: &mut Draw) -> Simplex {
        // The three offsets the noise draws and then discards: two draws a double.
        for _ in 0..6 {
            draw.next(26);
        }
        let mut permutation: [u8; 256] = std::array::from_fn(|i| i as u8);
        for i in 0..256 {
            permutation.swap(i, i + draw.below(256 - i as i32) as usize);
        }
        Simplex { permutation }
    }

    pub fn get(&self, x: f64, y: f64) -> f32 {
        let sqrt3 = 3f64.sqrt();
        let (f2, g2) = (0.5 * (sqrt3 - 1.0), (3.0 - sqrt3) / 6.0);
        let s = (x + y) * f2;
        let (i, j) = ((x + s).floor() as i32, (y + s).floor() as i32);
        let t = f64::from(i.wrapping_add(j)) * g2;
        let (x0, y0) = (x - (f64::from(i) - t), y - (f64::from(j) - t));
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let corners = [(0, 0, x0, y0), (i1, j1, x0 - f64::from(i1) + g2, y0 - f64::from(j1) + g2), (1, 1, x0 - 1.0 + 2.0 * g2, y0 - 1.0 + 2.0 * g2)];
        let permute = |v: i32| i32::from(self.permutation[(v & 0xff) as usize]);
        let sum: f64 = corners
            .iter()
            .map(|&(di, dj, x, y)| {
                let [gx, gy] = GRADIENT[(permute((i & 0xff) + di + permute((j & 0xff) + dj)) % 12) as usize];
                let t = 0.5 - x * x - y * y;
                let squared = t * t;
                if t < 0.0 { 0.0 } else { squared * squared * (gx * x + gy * y) }
            })
            .sum();
        (70.0 * sum) as f32
    }
}

pub static BIOME_INFO: LazyLock<Simplex> = LazyLock::new(|| Simplex::from_draw(&mut Draw::new(2345)));

/// Whether swamp grass is its darker colour in the column of world position `(x, z)`.
pub fn grass_patch(x: i32, z: i32) -> bool {
    f64::from(BIOME_INFO.get(f64::from(x) * 0.0225, f64::from(z) * 0.0225)) < -0.1
}

#[test]
fn the_noise_is_low_in_patches() {
    let low = (0..2000).filter(|i| grass_patch(i * 7, 300 - i * 3)).count();
    assert!((200..1200).contains(&low), "{low}");
    assert_eq!(grass_patch(40, -90), grass_patch(40, -90));
}
