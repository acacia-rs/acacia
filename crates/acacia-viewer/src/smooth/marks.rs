//! What the entities cast and carry besides their models this frame: shadows and name tags.

use acacia_render::entity::java::types;
use acacia_render::shadows::{self, Shadow};
use glam::DVec3;

use super::{Motion, OWN_HEAD_RADIUS, Smoother};
use crate::entities::Tracked;

/// A name tag this frame: its text, the feet under it (Java measures the reach from them), where
/// its line's top hangs and the reach in blocks.
pub struct NameTag<'a> {
    pub text: &'a str,
    pub feet: DVec3,
    pub anchor: DVec3,
    pub reach: f64,
}

impl Smoother {
    /// Name tags, where they hang this frame and how far they show: half a block over Java's
    /// name tag attachment (`types::name_tag`; the hitbox for kinds it lacks), a sneaker's within
    /// 32 blocks (`LivingEntityRenderer.shouldShowName`).
    pub fn name_tags(&self) -> Vec<NameTag<'_>> {
        let t = self.progress();
        let mut out = Vec::new();
        for (e, to) in &self.to {
            let Some(name) = e.name.as_deref() else { continue };
            let flag = |name: &str| e.facts.query(name).truthy();
            let pose = if flag("is_sneaking") { "crouching" } else if flag("is_sitting") { "sitting" } else { "standing" };
            let tag = types::name_tag(&e.kind, flag("is_baby"), pose);
            let height = tag.map_or_else(|| e.hitbox.map_or(1.8, |(_, h)| h), |tag| tag.height);
            let reach = if flag("is_sneaking") { 32.0 } else { tag.map_or(64.0, |tag| f64::from(tag.distance)) };
            let feet = self.blend(e.runtime_id, *to, t).position;
            out.push(NameTag { text: name, feet, anchor: feet + DVec3::Y * f64::from(height + 0.5), reach });
        }
        out
    }

    /// The round shadows under what is drawn from `camera`, each kind's as Java's renderer gives it
    /// ([`shadows::radius`]); a slime's size is read off its hitbox (0.52 blocks a size).
    pub fn shadows(&self, camera: DVec3) -> Vec<Shadow> {
        let t = self.progress();
        let cast = |(e, to): &(Tracked, Motion)| {
            if e.own_eyes.is_some_and(|eyes| eyes.distance(camera) <= OWN_HEAD_RADIUS) {
                return None;
            }
            let kind = if e.dropped.is_some() { "minecraft:item" } else { e.kind.as_str() };
            let baby = e.facts.query("is_baby").truthy();
            let size = e.hitbox.map_or(1.0, |(width, _)| (width / 0.52).round());
            let (radius, strength) = shadows::radius(kind, baby, size, e.facts.query("variant").num())?;
            let m = self.blend(e.runtime_id, *to, t);
            let feet = self.seated(e, m, t).map_or(m.position, |(seat, _)| seat.position);
            (radius > 0.0).then_some(Shadow { feet, radius, strength })
        };
        self.to.iter().filter_map(cast).collect()
    }
}
