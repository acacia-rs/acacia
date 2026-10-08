//! Entities from the bot's trackers to the renderer: [`Feed`] snapshots them on the bot thread;
//! [`crate::smooth`] blends between snapshots on the window thread.

use std::collections::HashMap;
use std::sync::Arc;

use acacia_bot::Bot;
use acacia_bot::proto::manual::Uuid;
use acacia_bot::proto::types::{MetadataDictionaryItemKey as Key, MetadataFlags1 as Flags};
use acacia_bot::state::{Entity, ITEM_KIND, Metadata, PlayerSkin};
use acacia_render::entity::{EntityInstance, EntityModels, Pose, Skin, SkinSource, Value};
use acacia_render::item::ItemKey;
use glam::DVec3;

/// Seconds between snapshots (the bot thread's report interval): one game tick.
pub const SNAPSHOT_SECS: f32 = 0.05;
const PLAYER: &str = "minecraft:player";

pub struct Tracked {
    pub runtime_id: u64,
    /// Eye position when this is the bot itself.
    pub own_eyes: Option<DVec3>,
    pub kind: String,
    pub facts: Facts,
    /// Degrees, like the instance's body yaw.
    pub head_yaw: f32,
    pub pitch: f32,
    /// Its pose is filled in per frame.
    pub instance: EntityInstance,
    /// A dropped item: drawn from its stack, and the instance has no layers.
    pub dropped: Option<DroppedStack>,
    /// Width and height of what the crosshair can hit; `None` for the bot and dropped items.
    pub hitbox: Option<(f32, f32)>,
}

pub struct DroppedStack {
    pub key: ItemKey,
    pub count: u16,
    /// Network id plus aux; see [`acacia_render::item::drop::Drop::seed`].
    pub seed: i32,
}

/// The entity data Molang queries read, copied out of the bot's metadata so the window thread
/// can answer them every frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Facts {
    /// Bit per entry of [`FLAGS`].
    flags: u16,
    variant: i32,
    mark_variant: i32,
    skin_id: i32,
    trade_tier: i32,
    color: i8,
}

const FLAGS: [(&str, Flags); 12] = [
    ("is_baby", Flags::BABY),
    ("is_sheared", Flags::SHEARED),
    ("is_saddled", Flags::SADDLED),
    ("is_tamed", Flags::TAMED),
    ("is_angry", Flags::ANGRY),
    ("is_chested", Flags::CHESTED),
    ("is_powered", Flags::POWERED),
    ("is_elder", Flags::ELDER),
    ("is_charging", Flags::CHARGE_ATTACK),
    ("is_casting", Flags::EVOKER_SPELL),
    ("is_sitting", Flags::SITTING),
    ("is_invisible", Flags::INVISIBLE),
];

impl Facts {
    fn of(meta: &Metadata) -> Facts {
        let set = meta.flags();
        Facts {
            flags: FLAGS.iter().enumerate().fold(0, |bits, (i, (_, flag))| bits | u16::from(set.contains(*flag)) << i),
            variant: meta.int(Key::Variant),
            mark_variant: meta.int(Key::MarkVariant),
            skin_id: meta.int(Key::SkinId),
            trade_tier: meta.int(Key::TradeTier),
            color: meta.color(),
        }
    }

    /// Answers a Molang query (without the `query.` prefix); the ones it does not know are 0.
    pub fn query(&self, name: &str) -> Value {
        Value::Num(match name {
            // TODO: track synced entity properties; until then every cow, pig and chicken is temperate.
            "property:minecraft:climate_variant" => return Value::Text("temperate".into()),
            "variant" => self.variant as f32,
            "mark_variant" => self.mark_variant as f32,
            "skin_id" => self.skin_id as f32,
            "trade_tier" => self.trade_tier as f32,
            "color" => f32::from(self.color),
            _ => FLAGS.iter().position(|(flag, _)| *flag == name).map_or(0.0, |i| f32::from(self.flags >> i & 1)),
        })
    }
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

pub fn wrap_degrees(angle: f32) -> f32 {
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
        if let Some(instance) = self.player(bot, uuid, feet, me.yaw, 1.0) {
            let own_eyes = Some(DVec3::new(eyes.x.into(), eyes.y.into(), eyes.z.into()));
            let (kind, facts) = (PLAYER.to_owned(), Facts::default());
            out.push(Tracked { runtime_id: me.runtime_entity_id, own_eyes, kind, facts, head_yaw: me.yaw, pitch: me.pitch, instance, dropped: None, hitbox: None });
        }

        let mut bodies = HashMap::with_capacity(out.len());
        for tracked in &mut out {
            let i = &mut tracked.instance;
            if let Some(&(body, at)) = self.bodies.get(&tracked.runtime_id) {
                i.yaw = turn_body(body, at.distance_squared(i.position) > MOVED_SQUARED, i.yaw, tracked.head_yaw);
            }
            bodies.insert(tracked.runtime_id, (i.yaw, i.position));
        }
        self.bodies = bodies;
        out
    }

    fn entity(&mut self, bot: &Bot, e: &Entity) -> Option<Tracked> {
        if e.metadata.flags().contains(Flags::INVISIBLE) {
            return None;
        }
        let feet = e.feet();
        let position = DVec3::new(feet.x.into(), feet.y.into(), feet.z.into());
        let facts = Facts::of(&e.metadata);
        let mut dropped = None;
        let (kind, instance) = if e.is_player() {
            (PLAYER.to_owned(), self.player(bot, e.uuid, position, e.yaw, e.metadata.scale())?)
        } else if e.kind == ITEM_KIND {
            let stack = e.item.as_ref().filter(|s| !s.is_empty())?;
            let key = ItemKey { name: bot.state().item_name(stack)?.to_owned(), aux: stack.metadata, block: crate::control::block_of(bot, stack) };
            dropped = Some(DroppedStack { key, count: stack.count, seed: stack.network_id.wrapping_add(stack.metadata as i32) });
            let instance = EntityInstance { layers: Arc::from([]), skin: None, position, yaw: 0.0, scale: 1.0, pose: Pose::default(), frame: None };
            (e.kind.clone(), instance)
        } else {
            let (layers, scale) = self.models.appearance(&e.kind, &|name| facts.query(name))?;
            let instance = EntityInstance { layers, skin: None, position, yaw: e.yaw, scale: scale * e.metadata.scale(), pose: Pose::default(), frame: None };
            (e.kind.clone(), instance)
        };
        let hitbox = e.metadata.bounding_box().filter(|_| dropped.is_none());
        Some(Tracked { runtime_id: e.runtime_id, own_eyes: None, kind, facts, head_yaw: e.head_yaw, pitch: e.pitch, instance, dropped, hitbox })
    }

    fn player(&mut self, bot: &Bot, uuid: Option<Uuid>, position: DVec3, yaw: f32, scale: f32) -> Option<EntityInstance> {
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
        Some(EntityInstance { layers, skin: skin.map(|(s, _)| s), position, yaw, scale, pose: Pose::default(), frame: None })
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
