//! What the sprite pass draws of either style's particles this frame: a quad's centre, size,
//! sheet rectangle, colour, light and blend, `partial` of a tick past the last tick.

use glam::DVec3;

use super::java::behaviour::Behaviour;
use super::java::Particle;
use super::sheet::Sheet;
use super::sprite::{Blend, Light, Sprite};
use super::{Particles, Style};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Draw {
    pub at: DVec3,
    /// Half the quad's side, in blocks.
    pub size: f32,
    pub uv: [f32; 4],
    pub color: [f32; 4],
    /// (block, sky) light levels.
    pub light: [f32; 2],
    pub blend: Blend,
}

impl Particles {
    /// `light` gives (block, sky) at a world position.
    pub fn draws(&self, sheet: &Sheet, partial: f64, light: impl Fn(DVec3) -> [f32; 2]) -> Vec<Draw> {
        match self.style {
            Style::Java => self.java.particles.iter().filter_map(|p| java(p, sheet, partial, &light)).collect(),
            Style::Bedrock => self.sprites.iter().filter_map(|s| bedrock(s, sheet, partial, &light)).collect(),
        }
    }
}

fn bedrock(s: &Sprite, sheet: &Sheet, partial: f64, light: &impl Fn(DVec3) -> [f32; 2]) -> Option<Draw> {
    if !s.visible() {
        return None;
    }
    let frames = sheet.frames(s.set);
    let uv = *frames.get(s.frame_index(frames.len()))?;
    let at = s.previous.lerp(s.position, partial);
    let [block, sky] = light(at);
    let light = match s.light {
        Light::World => [block, sky],
        Light::Full => [15.0, 15.0],
        Light::Ramp => [(block + s.life() * 15.0).min(15.0), sky],
        Light::Block => [15.0, sky],
    };
    Some(Draw { at, size: s.quad_size(), uv, color: s.tint(), light, blend: s.blend })
}

/// `SingleQuadParticle.extract`: the quad at its interpolated place, `getQuadSize(a)` and
/// `getLightCoords(a)`; the roll some classes keep is not drawn.
fn java(p: &Particle, sheet: &Sheet, partial: f64, light: &impl Fn(DVec3) -> [f32; 2]) -> Option<Draw> {
    if p.no_render || p.removed {
        return None;
    }
    let uv = sheet.named(p.sprite?)?;
    let a = partial as f32;
    let at = p.previous.lerp(p.position, partial);
    let mut color = p.rgba;
    if let Behaviour::Transition { from, to } = p.behaviour {
        let t = (p.age as f32 + a) / (p.lifetime as f32 + 1.0);
        (0..3).for_each(|i| color[i] = from[i] + (to[i] - from[i]) * t);
    }
    // `getLayer`: TRANSLUCENT for these classes, OPAQUE (cut out) for the rest drawn here.
    let blend = if matches!(p.behaviour, Behaviour::Campfire | Behaviour::SimpleAnimated { .. }) { Blend::Alpha } else { Blend::Cutout };
    Some(Draw { at, size: p.quad_size(a), uv, color, light: p.light(a, light(at)), blend })
}
