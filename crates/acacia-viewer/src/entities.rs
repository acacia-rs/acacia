//! Entities from the bot's trackers to the renderer: [`Feed`] snapshots them on the bot thread,
//! [`Smoother`] blends between snapshots on the window thread.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use acacia_bot::Bot;
use acacia_bot::proto::manual::Uuid;
use acacia_bot::proto::types::MetadataFlags1 as Flags;
use acacia_bot::state::{Entity, EntityMeta, PlayerSkin};
use acacia_render::entity::{EntityInstance, EntityModels, Skin, SkinSource, Value};
use glam::DVec3;

/// Seconds between snapshots (the bot thread's report interval).
pub const SNAPSHOT_SECS: f32 = 0.05;
/// The bot's own body is hidden while the camera is this close to its eyes.
const OWN_HEAD_RADIUS: f64 = 0.6;

pub struct Tracked {
    pub runtime_id: u64,
    /// Eye position when this is the bot itself.
    pub own_eyes: Option<DVec3>,
    pub instance: EntityInstance,
}

/// Molang queries (without the `query.` prefix) the bot's entity data can answer; the rest are 0.
fn query(meta: &EntityMeta, name: &str) -> Value {
    Value::Num(match name {
        // TODO: track synced entity properties; until then every cow, pig and chicken is temperate.
        "property:minecraft:climate_variant" => return Value::Text("temperate".into()),
        "variant" => meta.variant as f32,
        "mark_variant" => meta.mark_variant as f32,
        "skin_id" => meta.skin_id as f32,
        "trade_tier" => meta.trade_tier as f32,
        "color" => f32::from(meta.color),
        _ => f32::from(u8::from(flag(name).is_some_and(|f| meta.flags.contains(f)))),
    })
}

fn flag(query: &str) -> Option<Flags> {
    Some(match query {
        "is_baby" => Flags::BABY,
        "is_sheared" => Flags::SHEARED,
        "is_saddled" => Flags::SADDLED,
        "is_tamed" => Flags::TAMED,
        "is_angry" => Flags::ANGRY,
        "is_chested" => Flags::CHESTED,
        "is_powered" => Flags::POWERED,
        "is_elder" => Flags::ELDER,
        "is_charging" => Flags::CHARGE_ATTACK,
        "is_casting" => Flags::EVOKER_SPELL,
        "is_sitting" => Flags::SITTING,
        "is_invisible" => Flags::INVISIBLE,
        _ => return None,
    })
}

pub struct Feed {
    models: Arc<EntityModels>,
    /// Render copies of the bot's skins, replaced when the bot's `Arc` changes.
    skins: HashMap<Uuid, (Arc<PlayerSkin>, Arc<Skin>, bool)>,
    /// Body yaw and position at the last snapshot, by runtime id.
    bodies: HashMap<u64, (f32, DVec3)>,
}

/// Share of the way the body turns towards the server's yaw per snapshot while the entity moves.
const BODY_TURN: f32 = 0.3;
/// Degrees the head may be turned away from the body before the body follows.
const MAX_HEAD_TURN: f32 = 75.0;
const MOVED_SQUARED: f64 = 0.0025 * 0.0025;

fn wrap_degrees(angle: f32) -> f32 {
    (angle + 540.0).rem_euclid(360.0) - 180.0
}

/// The body yaw one snapshot on. Servers sync where a mob heads and where it looks, not where its
/// body points: like the vanilla client, the body eases towards the heading while moving and is
/// dragged along once the head is turned too far.
fn turn_body(body: f32, moved: bool, yaw: f32, head_yaw: f32) -> f32 {
    let body = if moved { body + wrap_degrees(yaw - body) * BODY_TURN } else { body };
    head_yaw - wrap_degrees(head_yaw - body).clamp(-MAX_HEAD_TURN, MAX_HEAD_TURN)
}

impl Feed {
    pub fn new(models: Arc<EntityModels>) -> Self {
        Feed { models, skins: HashMap::new(), bodies: HashMap::new() }
    }

    pub fn snapshot(&mut self, bot: &Bot) -> Vec<Tracked> {
        let state = bot.state();
        self.skins.retain(|uuid, _| state.skins.get(uuid).is_some());
        let mut out: Vec<Tracked> = state.entities.iter().filter_map(|e| self.entity(bot, e)).collect();

        let me = &state.player;
        let uuid = state.player_list.iter().find(|p| p.entity_unique_id == me.unique_entity_id).map(|p| p.uuid);
        let eyes = me.eye_position();
        let feet = DVec3::new(me.position.x.into(), me.position.y.into(), me.position.z.into());
        if let Some(instance) = self.player(bot, uuid, feet, [me.yaw, me.yaw, me.pitch], 1.0) {
            let own_eyes = Some(DVec3::new(eyes.x.into(), eyes.y.into(), eyes.z.into()));
            out.push(Tracked { runtime_id: me.runtime_entity_id, own_eyes, instance });
        }

        let mut bodies = HashMap::with_capacity(out.len());
        for tracked in &mut out {
            let i = &mut tracked.instance;
            if let Some(&(body, at)) = self.bodies.get(&tracked.runtime_id) {
                i.yaw = turn_body(body, at.distance_squared(i.position) > MOVED_SQUARED, i.yaw, i.head_yaw);
            }
            bodies.insert(tracked.runtime_id, (i.yaw, i.position));
        }
        self.bodies = bodies;
        out
    }

    fn entity(&mut self, bot: &Bot, e: &Entity) -> Option<Tracked> {
        if e.meta.is_invisible() {
            return None;
        }
        let feet = e.feet();        let position = DVec3::new(feet.x.into(), feet.y.into(), feet.z.into());
        let instance = if e.is_player() {
            self.player(bot, e.uuid, position, [e.yaw, e.head_yaw, e.pitch], e.meta.scale)?
        } else {
            let (layers, scale) = self.models.appearance(&e.kind, &|name| query(&e.meta, name))?;
            EntityInstance { layers, skin: None, position, yaw: e.yaw, head_yaw: e.head_yaw, pitch: e.pitch, scale: scale * e.meta.scale }
        };
        Some(Tracked { runtime_id: e.runtime_id, own_eyes: None, instance })
    }

    fn player(&mut self, bot: &Bot, uuid: Option<Uuid>, position: DVec3, [yaw, head_yaw, pitch]: [f32; 3], scale: f32) -> Option<EntityInstance> {
        let skin = uuid.and_then(|uuid| {
            let source = bot.state().skins.get(&uuid)?;
            if !self.skins.get(&uuid).is_some_and(|(from, ..)| Arc::ptr_eq(from, source)) {
                let skin = Skin::new(SkinSource {
                    size: (source.texture.width, source.texture.height),
                    rgba: &source.texture.rgba,
                    face: source.face.as_ref().map(|f| ((f.width, f.height), &f.rgba[..])),
                    resource_patch: &source.resource_patch,
                    geometry_data: &source.geometry_data,
                });
                self.skins.insert(uuid, (source.clone(), Arc::new(skin), source.slim));
            }
            let (_, skin, slim) = &self.skins[&uuid];
            Some((skin.clone(), *slim))
        });
        let layers = self.models.player(skin.as_ref().map(|(s, slim)| (&**s, *slim)))?;
        Some(EntityInstance { layers, skin: skin.map(|(s, _)| s), position, yaw, head_yaw, pitch, scale })
    }
}

/// Blends each entity from where it was drawn at the previous snapshot towards the newest one.
#[derive(Default)]
pub struct Smoother {
    from: HashMap<u64, EntityInstance>,
    to: Vec<Tracked>,
    since: Option<Instant>,
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    a + wrap_degrees(b - a) * t
}

impl Smoother {
    pub fn push(&mut self, camera: DVec3, snapshot: Vec<Tracked>) {
        self.from = self.to.iter().zip(self.frame(camera, true)).map(|(t, shown)| (t.runtime_id, shown)).collect();
        self.to = snapshot;
        self.since = Some(Instant::now());
    }

    pub fn instances(&self, camera: DVec3) -> Vec<EntityInstance> {
        self.frame(camera, false)
    }

    /// `keep_hidden` keeps the bot's own body in the list, so the result lines up with `to`.
    fn frame(&self, camera: DVec3, keep_hidden: bool) -> Vec<EntityInstance> {
        let t = self.since.map_or(1.0, |s| (s.elapsed().as_secs_f32() / SNAPSHOT_SECS).min(1.0));
        let visible = |e: &&Tracked| keep_hidden || e.own_eyes.is_none_or(|eyes| eyes.distance(camera) > OWN_HEAD_RADIUS);
        let blend = |e: &Tracked| {
            let to = &e.instance;
            let Some(from) = self.from.get(&e.runtime_id) else { return to.clone() };
            EntityInstance {
                position: from.position.lerp(to.position, f64::from(t)),
                yaw: lerp_angle(from.yaw, to.yaw, t),
                head_yaw: lerp_angle(from.head_yaw, to.head_yaw, t),
                pitch: lerp_angle(from.pitch, to.pitch, t),
                ..to.clone()
            }
        };
        self.to.iter().filter(visible).map(blend).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bodies_follow_the_heading_and_a_far_turned_head() {
        // Standing still, the head looks around: the body stays until the head passes 75°.
        assert_eq!(turn_body(0.0, false, 0.0, 60.0), 0.0);
        assert_eq!(turn_body(0.0, false, 0.0, 100.0), 25.0);
        assert_eq!(turn_body(0.0, false, 0.0, 200.0), 275.0);
        // Walking, it eases towards the heading, the short way round (356°).
        assert_eq!(turn_body(350.0, true, 10.0, 0.0), -4.0);
        let settled = (0..40).fold(0.0, |body, _| turn_body(body, true, 90.0, 90.0));
        assert!((settled - 90.0).abs() < 0.01, "{settled}");
    }
}
