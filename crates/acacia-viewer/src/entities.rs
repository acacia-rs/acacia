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
}

impl Feed {
    pub fn new(models: Arc<EntityModels>) -> Self {
        Feed { models, skins: HashMap::new() }
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
        out
    }

    fn entity(&mut self, bot: &Bot, e: &Entity) -> Option<Tracked> {
        if e.meta.is_invisible() {
            return None;
        }
        let feet = e.feet();
        let position = DVec3::new(feet.x.into(), feet.y.into(), feet.z.into());
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
    a + ((b - a + 540.0).rem_euclid(360.0) - 180.0) * t
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
