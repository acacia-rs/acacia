//! Entities between snapshots on the window thread: [`Smoother`] blends each one from where it
//! was drawn at the previous snapshot towards the newest, and poses it for the frame.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use acacia_render::entity::{EntityInstance, EntityModels, Value, cape};
use acacia_render::item::drop::{self, Drop};
use acacia_render::item::{ItemModel, ItemModels, SHIELD, hand};
use acacia_render::shadows::Shadow;
use glam::DVec3;

use crate::entities::{SNAPSHOT_SECS, Tracked, wrap_degrees};
use crate::pick::EntityBox;

mod motion;
pub mod ropes;

use motion::{Motion, Walk};

/// The bot's own body is hidden while the camera is this close to its eyes.
const OWN_HEAD_RADIUS: f64 = 0.6;
/// `query.is_riding_any_entity_of_type` with one of its kinds, as the entity code asks it.
const RIDING_KIND: &str = "is_riding_any_entity_of_type:";
/// Degrees a head turns from its body.
const MAX_HEAD_TURN: f32 = 90.0;

/// The item in a hand as Molang names it: without the namespace, empty for none.
fn held_name(e: &Tracked, hand: usize) -> Value {
    Value::Text(e.held[hand].as_ref().map_or("", |(key, _)| key.name.trim_start_matches("minecraft:")).to_owned())
}

#[derive(Default)]
pub struct Smoother {
    models: Arc<EntityModels>,
    from: HashMap<u64, Motion>,
    to: Vec<(Tracked, Motion)>,
    since: Option<Instant>,
    /// When each entity was first seen, for `query.life_time`.
    born: HashMap<u64, Instant>,
    /// For the world shown; built with its look pack.
    items: Option<ItemModels>,
    /// This snapshot's dropped items that have a model, by runtime id.
    dropped: HashMap<u64, ItemModel>,
    /// This snapshot's held items that have a model, by runtime id.
    held: HashMap<u64, [Option<ItemModel>; 2]>,
    /// By runtime id: hurts seen, when the last began, and when the dying began.
    hurt: HashMap<u64, Hurt>,
}

#[derive(Default, Clone, Copy)]
struct Hurt {
    count: u32,
    at: Option<Instant>,
    dying_since: Option<Instant>,
}

/// Java's hurt time (10 ticks) and death topple: 90° by `sqrt((ticks - 1) / 20 * 1.6)`.
const HURT_SECS: f32 = 0.5;

fn topple_degrees(dying_secs: f32) -> f32 {
    ((dying_secs * 20.0 - 1.0) / 20.0 * 1.6).max(0.0).sqrt().min(1.0) * 90.0
}

impl Smoother {
    pub fn set_models(&mut self, models: Arc<EntityModels>) {
        self.models = models;
    }

    pub fn set_items(&mut self, items: ItemModels) {
        self.items = Some(items);
    }

    /// An item's model in the shown world's look (the held item's, for the hand).
    pub fn item(&mut self, key: &acacia_render::item::ItemKey, enchanted: bool) -> Option<ItemModel> {
        self.items.as_mut()?.get(key, enchanted)
    }

    pub fn push(&mut self, snapshot: Vec<Tracked>) {
        let t = self.progress();
        let now = Instant::now();
        let shown: HashMap<u64, Motion> = self.to.iter().map(|(e, to)| (e.runtime_id, self.blend(e.runtime_id, *to, t))).collect();
        let last: HashMap<u64, Motion> = self.to.iter().map(|(e, to)| (e.runtime_id, *to)).collect();
        self.born.retain(|id, _| last.contains_key(id));
        self.hurt.retain(|id, _| last.contains_key(id));
        self.to = snapshot
            .into_iter()
            .map(|e| {
                self.born.entry(e.runtime_id).or_insert(now);
                let hurt = self.hurt.entry(e.runtime_id).or_insert(Hurt { count: e.hurts, ..Hurt::default() });
                if e.hurts > hurt.count {
                    (hurt.count, hurt.at) = (e.hurts, Some(now));
                }
                hurt.dying_since = if e.dying { hurt.dying_since.or(Some(now)) } else { None };
                let walk = last.get(&e.runtime_id).map_or(Walk::default(), |before| {
                    let moved = e.instance.position - before.position;
                    before.walk.step(moved.x.hypot(moved.z) as f32)
                });
                let motion = Motion::of(&e, walk);
                (e, motion)
            })
            .collect();
        (self.dropped, self.held) = match &mut self.items {
            Some(items) => (
                self.to.iter().filter_map(|(e, _)| Some((e.runtime_id, e.dropped.as_ref().and_then(|d| items.get(&d.key, d.enchanted))?))).collect(),
                self.to.iter().map(|(e, _)| (e.runtime_id, e.held.each_ref().map(|h| h.as_ref().and_then(|(key, enchanted)| items.get(key, *enchanted))))).collect(),
            ),
            None => (HashMap::new(), HashMap::new()),
        };
        self.from = shown;
        self.since = Some(now);
    }

    fn progress(&self) -> f32 {
        self.since.map_or(1.0, |s| (s.elapsed().as_secs_f32() / SNAPSHOT_SECS).min(1.0))
    }

    fn blend(&self, id: u64, to: Motion, t: f32) -> Motion {
        self.from.get(&id).map_or(to, |from| from.towards(to, t))
    }

    pub fn instances(&self, camera: DVec3) -> Vec<EntityInstance> {
        let t = self.progress();
        let visible = |(e, _): &&(Tracked, Motion)| e.own_eyes.is_none_or(|eyes| eyes.distance(camera) > OWN_HEAD_RADIUS);
        let posed = |(e, to): &(Tracked, Motion)| {
            let m = self.blend(e.runtime_id, *to, t);
            let (m, vehicle) = self.seated(e, m, t).map_or((m, None), |(m, kind)| (m, Some(kind)));
            self.pieces(e, m, camera, None, vehicle)
        };
        self.to.iter().filter(visible).flat_map(posed).collect()
    }

    /// A rider's motion this frame, on its seat (turned with the vehicle where the seat holds it), and the vehicle's kind.
    fn seated(&self, e: &Tracked, m: Motion, t: f32) -> Option<(Motion, &str)> {
        let seat = e.seat?;
        let (vehicle, to) = self.to.iter().find(|(v, _)| v.runtime_id == seat.vehicle)?;
        let v = self.blend(vehicle.runtime_id, *to, t);
        let offset = glam::Quat::from_rotation_y(-v.yaw.to_radians()) * seat.offset;
        Some((Motion { position: v.position + offset.as_dvec3(), yaw: seat.turn.map_or(m.yaw, |turn| v.yaw + turn), walk: Walk::default(), ..m }, &vehicle.kind))
    }

    /// The own player standing still at `place` (model space to camera-relative space), its head
    /// turned by `head_yaw` and `pitch` degrees: the figure in the inventory screen.
    pub fn portrait(&self, place: glam::Mat4, head_yaw: f32, pitch: f32, camera: DVec3) -> Vec<EntityInstance> {
        let Some((e, _)) = self.to.iter().find(|(e, _)| e.own_eyes.is_some()) else { return Vec::new() };
        let still = Motion { position: camera, yaw: 0.0, head_yaw, pitch, walk: Walk::default() };
        self.pieces(e, still, camera, Some(place), None)
    }

    /// One entity's instances (itself, what it wears and holds) at `m`, or at `place` instead;
    /// `vehicle` is the kind it rides.
    fn pieces(&self, e: &Tracked, m: Motion, camera: DVec3, place: Option<glam::Mat4>, vehicle: Option<&str>) -> Vec<EntityInstance> {
        let life = self.born.get(&e.runtime_id).map_or(0.0, |b| b.elapsed().as_secs_f32());
        if let Some(stack) = &e.dropped {
            let (Some(model), Some(items)) = (self.dropped.get(&e.runtime_id), &self.items) else { return Vec::new() };
            let age_ticks = life / SNAPSHOT_SECS;
            let at = Drop { feet: m.position, count: stack.count, seed: stack.seed, age_ticks, bob_offset: drop::bob_offset(e.runtime_id) };
            return drop::instances(model, items.dropped(), &at);
        }
        let query = |name: &str| {
            Value::Num(match name {
                "life_time" => life,
                "modified_distance_moved" => m.walk.distance,
                "modified_move_speed" => m.walk.speed,
                "target_x_rotation" => m.pitch,
                "target_y_rotation" => wrap_degrees(m.head_yaw - m.yaw).clamp(-MAX_HEAD_TURN, MAX_HEAD_TURN),
                "is_riding" => f32::from(u8::from(vehicle.is_some())),
                _ if name.starts_with(RIDING_KIND) => f32::from(u8::from(vehicle == name.strip_prefix(RIDING_KIND))),
                "is_on_ground" => 1.0,
                // Blocks per model pixel.
                "model_scale" => e.instance.scale / 16.0,
                "is_alive" => f32::from(u8::from(!e.dying)),
                "get_equipped_item_name" | "get_equipped_item_name:main_hand" => return held_name(e, 0),
                "get_equipped_item_name:off_hand" => return held_name(e, 1),
                _ => return e.facts.query(name),
            })
        };
        let mut pose = e.instance.layers.first().map(|l| self.models.pose(&e.kind, l.model, &query)).unwrap_or_default();
        // The off hand's shield is raised before the main one's.
        let raised = [1, 0].into_iter().find(|&side| e.facts.blocking() && e.held[side].as_ref().is_some_and(|(key, _)| key.name == SHIELD));
        if let Some(side) = raised {
            acacia_render::item::raise_shield_arm(&mut pose, side == 1);
        }
        let mut out = Vec::with_capacity(2);
        let status = self.hurt.get(&e.runtime_id).copied().unwrap_or_default();
        let dying = status.dying_since.map(|since| since.elapsed().as_secs_f32());
        let hurt = dying.is_some() || status.at.is_some_and(|at| at.elapsed().as_secs_f32() < HURT_SECS);
        // The body transform the entity pass gives this instance, toppled while dying.
        let s = e.instance.scale;
        let topple = glam::Mat4::from_rotation_z(-dying.map_or(0.0, topple_degrees).to_radians());
        let stood = glam::Mat4::from_translation((m.position - camera).as_vec3()) * glam::Mat4::from_rotation_y(-m.yaw.to_radians()) * topple * glam::Mat4::from_scale(glam::Vec3::new(s, s, -s));
        let body = place.map_or(stood, |place| place * glam::Mat4::from_scale(glam::Vec3::splat(s)));
        let frame = (dying.is_some() || place.is_some()).then_some(body);
        let held = self.held.get(&e.runtime_id).into_iter().flatten().enumerate();
        for (left, item) in held.filter_map(|(side, item)| Some((side == 1, item.as_ref()?))) {
            let mesh = e.instance.layers.first().and_then(|l| self.models.models().get(l.model as usize)).map(|model| &model.mesh);
            let skin_mesh = e.instance.skin.as_ref().and_then(|skin| skin.mesh.as_ref());
            if let Some(hand) = skin_mesh.or(mesh).and_then(|mesh| mesh.hand(&pose, left)) {
                out.push(hand::third_person(item, body, hand, left, raised == Some(usize::from(left)), m.position + DVec3::Y));
            }
        }
        let glint = |enchanted: bool| enchanted.then_some(acacia_render::glint::Glint::Armor);
        let worn = e.armor.iter().map(|(layers, enchanted)| EntityInstance { layers: layers.clone(), skin: None, position: m.position, yaw: m.yaw, pose: pose.clone(), frame, hurt, glint: glint(*enchanted), ..e.instance.clone() });
        out.extend(worn);
        let cape = e.cape.iter().map(|cape| EntityInstance { layers: cape::layers(), skin: Some(cape.clone()), position: m.position, yaw: m.yaw, pose: cape::pose(&pose), frame, hurt, ..e.instance.clone() });
        out.extend(cape);
        out.push(EntityInstance { position: m.position, yaw: m.yaw, pose, frame, hurt, ..e.instance.clone() });
        out
    }

    /// Name tags and where they hang this frame: half a block over the entity's box.
    pub fn name_tags(&self) -> Vec<(&str, DVec3)> {
        let t = self.progress();
        let mut out = Vec::new();
        for (e, to) in &self.to {
            let Some(name) = e.name.as_deref() else { continue };
            let height = e.hitbox.map_or(1.8, |(_, h)| h);
            out.push((name, self.blend(e.runtime_id, *to, t).position + DVec3::Y * f64::from(height + 0.5)));
        }
        out
    }

    /// Where the hittable entities are drawn this frame.
    pub fn hitboxes(&self) -> Vec<EntityBox> {
        let t = self.progress();
        let hittable = |(e, to): &(Tracked, Motion)| {
            let (width, height) = e.hitbox?;
            Some(EntityBox { runtime_id: e.runtime_id, feet: self.blend(e.runtime_id, *to, t).position, width, height })
        };
        self.to.iter().filter_map(hittable).collect()
    }

    /// The round shadows under what is drawn from `camera`. Java gives each kind a radius; the
    /// pack has none, so it is taken from the hitbox (players 0.5, cows 0.7, chickens 0.3 as in
    /// Java), 0.15 under dropped items, and none for the kinds Java gives none.
    pub fn shadows(&self, camera: DVec3) -> Vec<Shadow> {
        const NONE: [&str; 4] = ["minecraft:armor_stand", "minecraft:painting", "minecraft:lightning_bolt", "minecraft:arrow"];
        let t = self.progress();
        let cast = |(e, to): &(Tracked, Motion)| {
            let radius = match (&e.dropped, e.hitbox, e.own_eyes) {
                (Some(_), ..) => 0.15,
                (None, Some((width, _)), _) => (width * 0.8).min(0.8),
                (None, None, Some(eyes)) if eyes.distance(camera) > OWN_HEAD_RADIUS => 0.5,
                _ => return None,
            };
            let m = self.blend(e.runtime_id, *to, t);
            let feet = self.seated(e, m, t).map_or(m.position, |(seat, _)| seat.position);
            (!NONE.contains(&e.kind.as_str())).then_some(Shadow { feet, radius })
        };
        self.to.iter().filter_map(cast).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legs_swing_while_walking_and_settle_when_standing() {
        let walking = (0..40).fold(Walk::default(), |w, _| w.step(0.1));
        assert!((walking.speed - 0.4).abs() < 1e-3 && walking.distance > 10.0, "{walking:?}");
        let stopped = (0..40).fold(walking, |w, _| w.step(0.0));
        assert!(stopped.speed < 1e-3 && stopped.distance - walking.distance < 1.0, "{stopped:?}");
    }

    #[test]
    fn the_dead_topple_over_in_under_a_second() {
        // Java: nothing on the first tick, then sqrt((ticks - 1) / 20 * 1.6), flat after 13.5 ticks.
        assert_eq!((topple_degrees(0.0), topple_degrees(0.05)), (0.0, 0.0));
        assert!((topple_degrees(0.3) - (5.0f32 / 20.0 * 1.6).sqrt() * 90.0).abs() < 1e-3);
        assert_eq!(topple_degrees(0.7), 90.0);
    }
}
