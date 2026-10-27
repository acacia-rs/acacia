//! The UI pass's blends: one pipeline per [`BLENDS`], drawn over runs of quads.

use acacia_ui::Quad;
use acacia_ui::draw::Blend;

pub const BLENDS: [Blend; 5] = [Blend::Alpha, Blend::Invert, Blend::Darken, Blend::Add, Blend::Premultiplied];

/// Java's blend functions: `TRANSLUCENT`, `INVERT`, the vignette's, `ADDITIVE` and
/// `TRANSLUCENT_PREMULTIPLIED_ALPHA`.
pub fn blend_state(blend: Blend) -> wgpu::BlendState {
    use wgpu::{BlendComponent as C, BlendFactor as F, BlendOperation::Add};
    let c = |src_factor, dst_factor| C { src_factor, dst_factor, operation: Add };
    match blend {
        Blend::Alpha => wgpu::BlendState::ALPHA_BLENDING,
        Blend::Invert => wgpu::BlendState { color: c(F::OneMinusDst, F::OneMinusSrc), alpha: c(F::One, F::Zero) },
        Blend::Darken => wgpu::BlendState { color: c(F::Zero, F::OneMinusSrc), alpha: c(F::Zero, F::One) },
        Blend::Add => wgpu::BlendState { color: c(F::One, F::One), alpha: c(F::One, F::One) },
        Blend::Premultiplied => wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
    }
}

/// Consecutive quads of one blend, as instance ranges with that blend's index in [`BLENDS`].
pub fn runs(quads: &[Quad]) -> Vec<(std::ops::Range<u32>, usize)> {
    let mut out: Vec<(std::ops::Range<u32>, usize)> = Vec::new();
    for (i, q) in quads.iter().enumerate() {
        let blend = BLENDS.iter().position(|&b| b == q.blend).unwrap_or(0);
        match out.last_mut() {
            Some((range, b)) if *b == blend => range.end = i as u32 + 1,
            _ => out.push((i as u32..i as u32 + 1, blend)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blend_change_starts_a_run() {
        let q = |blend| Quad { blend, ..Quad::plain([0.0; 4], [0.0; 4], [255; 4]) };
        let quads = [q(Blend::Alpha), q(Blend::Alpha), q(Blend::Invert), q(Blend::Alpha)];
        assert_eq!(runs(&quads), vec![(0..2, 0), (2..3, 1), (3..4, 0)]);
    }
}
