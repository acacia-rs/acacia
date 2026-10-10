//! Fishing lines and leads between the frame's entities ([`acacia_render::rope`]).

use acacia_bot::Bot;
use acacia_bot::proto::types::MetadataDictionaryItemKey as Key;
use acacia_bot::state::{Entity, FISHING_HOOK_KIND};
use acacia_render::Camera;
use acacia_render::entity::EntityInstance;
use acacia_render::rope::{self, Rope};
use glam::{DVec3, Vec3};

use super::{Motion, OWN_HEAD_RADIUS, Smoother};
use crate::entities::Tracked;

/// A rope from an entity to the one holding it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tether {
    /// The holder's runtime id.
    pub holder: u64,
    pub rope: Rope,
}

/// The rope `e` hangs from: a fishing hook's line to its owner, any other entity's lead to its
/// holder (a mob, a player or a fence knot).
pub fn tether(bot: &Bot, e: &Entity) -> Option<Tether> {
    let (key, rope) = if e.kind == FISHING_HOOK_KIND { (Key::OwnerEid, Rope::FishingLine) } else { (Key::LeadHolderEid, Rope::Lead) };
    let unique = e.metadata.entity(key)?;
    let state = bot.state();
    let own = (state.player.unique_entity_id == unique).then_some(state.player.runtime_entity_id);
    Some(Tether { holder: own.or_else(|| Some(state.entities.by_unique(unique)?.runtime_id))?, rope })
}

/// Where the holder's hand is from its feet: to its right and ahead, in blocks (right, up, ahead).
/// Java measures the same from the arm's pose; a rod's tip is further out than a fist.
fn reach(rope: Rope) -> Vec3 {
    match rope {
        Rope::FishingLine => Vec3::new(0.35, 1.1, 0.8),
        Rope::Lead => Vec3::new(0.35, 0.9, 0.3),
    }
}

/// The first-person hand's place against the camera (right, up, ahead).
const OWN_HAND: Vec3 = Vec3::new(0.45, -0.35, 0.7);
/// The knot on a fence: the lead ties to the entity itself.
const KNOT: &str = "minecraft:leash_knot";
/// How far up a leashed mob's box its lead ties.
const TIE_HEIGHT: f32 = 0.7;

impl Smoother {
    /// Everything the entities draw this frame: themselves and the ropes between them.
    pub fn scene(&self, camera: &Camera) -> Vec<EntityInstance> {
        let mut out = self.instances(camera.position);
        let t = self.progress();
        let shown = |(e, to): &(Tracked, Motion)| self.seated(e, self.blend(e.runtime_id, *to, t), t).map_or_else(|| self.blend(e.runtime_id, *to, t), |(m, _)| m);
        for (e, tether) in self.to.iter().filter_map(|pair| Some((pair, pair.0.tether?))) {
            let Some(holder) = self.to.iter().find(|(h, _)| h.runtime_id == tether.holder) else { continue };
            let (end, held) = (shown(e), shown(holder));
            let tie = e.0.hitbox.filter(|_| tether.rope == Rope::Lead).map_or(0.0, |(_, height)| height * TIE_HEIGHT);
            let first_person = holder.0.own_eyes.is_some_and(|eyes| eyes.distance(camera.position) <= OWN_HEAD_RADIUS);
            let hand = if first_person {
                camera.position + (camera.right() * OWN_HAND.x + Vec3::Y * OWN_HAND.y + camera.forward() * OWN_HAND.z).as_dvec3()
            } else if holder.0.kind == KNOT {
                held.position
            } else {
                let (sin, cos) = held.yaw.to_radians().sin_cos();
                // Yaw 0 faces +z, and the right hand is then towards -x.
                let (ahead, right, r) = (Vec3::new(-sin, 0.0, cos), Vec3::new(-cos, 0.0, -sin), reach(tether.rope));
                held.position + (right * r.x + Vec3::Y * r.y + ahead * r.z).as_dvec3()
            };
            out.extend(rope::instances(tether.rope, end.position + DVec3::Y * f64::from(tie), hand, camera.position));
        }
        out
    }
}
