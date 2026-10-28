//! Java's menu blur (`post_effect/blur.json` through `post/box_blur.fsh`): what is drawn before a
//! screen's `blurBeforeThisStratum` is box-blurred, horizontally then vertically, three times over,
//! by the `menuBackgroundBlurriness` option's radius. The renderer runs [`Blur::passes`] with
//! [`taps`]' samples; both are here so a harness can hold them against the game's post chain.

/// Java's default `menuBackgroundBlurriness`; 0 turns the blur off.
pub const DEFAULT_RADIUS: u32 = 5;

/// A blur of the frame under the GUI quads from `at` on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blur {
    /// How many of the frame's quads are drawn before it.
    pub at: usize,
    /// `MenuBlurRadius`: the option's value (each pass's own `Radius` is 0, so this one counts).
    pub radius: u32,
}

impl Blur {
    /// The post chain's passes in order: each the direction it samples along, in texels.
    pub fn passes() -> [[f32; 2]; 6] {
        [[1.0, 0.0], [0.0, 1.0], [1.0, 0.0], [0.0, 1.0], [1.0, 0.0], [0.0, 1.0]]
    }
}

/// `box_blur.fsh`'s samples for `radius`: offsets in texels along the pass's direction and their
/// weights. Bilinear sampling between two texels stands for both, hence the step of 2 and the
/// last sample's half weight.
pub fn taps(radius: u32) -> Vec<(f32, f32)> {
    let r = radius as f32;
    let norm = r + 0.5;
    let mut out = Vec::new();
    let mut a = -r + 0.5;
    while a <= r {
        out.push((a, 1.0 / norm));
        a += 2.0;
    }
    out.push((r, 0.5 / norm));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_radius_takes_six_samples_weighing_one() {
        let t = taps(DEFAULT_RADIUS);
        assert_eq!(t.iter().map(|&(a, _)| a).collect::<Vec<_>>(), [-4.5, -2.5, -0.5, 1.5, 3.5, 5.0]);
        assert!((t.iter().map(|&(_, w)| w).sum::<f32>() - 1.0).abs() < 1e-6);
    }
}
