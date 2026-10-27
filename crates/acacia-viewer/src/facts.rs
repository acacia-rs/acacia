//! The entity data Molang queries read, copied out of the bot's metadata so the window thread
//! can answer them every frame.

use acacia_bot::proto::types::{
    MetadataDictionaryItemKey as Key, MetadataDictionaryItemValueDefault as Plain, MetadataFlags1 as Flags, MetadataFlags2,
};
use std::sync::Arc;

use acacia_bot::state::{Metadata, Property};
use acacia_render::entity::Value;
use acacia_render::entity::boat::ROW_TIME;
use acacia_render::entity::java::bedrock::{CREEPER_SWELL, FREEZING};

#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// Synced actor properties by name (`minecraft:climate_variant`): an enum's value as text, the
    /// rest as numbers.
    properties: Arc<[(Box<str>, Value)]>,
    /// Bit per entry of [`FLAGS`], then [`BLOCKING_BIT`].
    flags: u32,
    variant: i32,
    mark_variant: i32,
    skin_id: i32,
    trade_tier: i32,
    color: i8,
    color2: i8,
    /// `minecraft:health`; 0 when the server sent none.
    health: f32,
    /// `CreeperSwell`: ticks into a creeper's fuse.
    swell: i32,
    /// `FreezingEffectStrength`: 0 to 1, fully frozen at 1.
    freezing: f32,
    /// A boat's `PaddleTimeLeft` and `PaddleTimeRight`, as the model's rowing time ([`ROW_SWEEP`]).
    row_time: [f32; 2],
}

/// Radians of paddle sweep per unit of a boat's paddle time. BDS adds 0.04 a tick while a paddle
/// rows (seen live, 1.26.50) and a stroke is 10 ticks (acacia-physics `boat`): one sweep per
/// stroke is assumed, the Bedrock client's own factor is unmeasured.
const ROW_SWEEP: f32 = std::f32::consts::TAU / 0.4;

const SNEAKING_BIT: usize = 0;

/// `is_shaking` is the actor flag the protocol names `VIBRATING` (bit 40, Bedrock's `SHAKING`).
const FLAGS: [(&str, Flags); 16] = [
    ("is_sneaking", Flags::SNEAKING),
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
    ("is_shaking", Flags::VIBRATING),
    ("is_converting", Flags::CONVERTING),
    ("is_ignited", Flags::IGNITED),
];

/// Raising a shield, from the extended flags.
const BLOCKING_BIT: usize = FLAGS.len();

impl Facts {
    /// `health` is the entity's `minecraft:health` attribute, when the server sent it.
    pub fn of<'a>(meta: &Metadata, health: Option<f32>, properties: impl Iterator<Item = (&'a str, Property<'a>)>) -> Facts {
        let set = meta.flags();
        let blocking = u32::from(meta.flags_extended().contains(MetadataFlags2::BLOCKING)) << BLOCKING_BIT;
        let property = |value| match value {
            Property::Enum(text) => Value::Text(text.into()),
            Property::Int(n) => Value::Num(n as f32),
            Property::Float(n) => Value::Num(n),
            Property::Bool(b) => Value::Num(f32::from(u8::from(b))),
        };
        Facts {
            properties: properties.map(|(name, value)| (name.into(), property(value))).collect(),
            flags: FLAGS.iter().enumerate().fold(blocking, |bits, (i, (_, flag))| bits | u32::from(set.contains(*flag)) << i),
            variant: meta.int(Key::Variant),
            mark_variant: meta.int(Key::MarkVariant),
            skin_id: meta.int(Key::SkinId),
            trade_tier: meta.int(Key::TradeTier),
            color: meta.color(),
            color2: match meta.plain(Key::Color2) {
                Some(Plain::Byte(v)) => *v,
                _ => 0,
            },
            health: health.unwrap_or(0.0),
            swell: meta.int(Key::CreeperSwell),
            freezing: meta.float(Key::FreezingEffectStrength),
            row_time: [Key::PaddleTimeLeft, Key::PaddleTimeRight].map(|key| meta.float(key) * ROW_SWEEP),
        }
    }

    /// The own player's, whose metadata the server does not send back: only what its input says.
    pub fn own(sneaking: bool, blocking: bool) -> Facts {
        Facts { flags: u32::from(blocking) << BLOCKING_BIT | u32::from(sneaking) << SNEAKING_BIT, ..Facts::default() }
    }

    /// A shield holder blocks while sneaking: BDS left the flag unset on a sneaking player
    /// with a shield (2026-10-10, 1.26.50).
    pub fn with_a_shield(self) -> Facts {
        let sneaking = self.flags >> SNEAKING_BIT & 1;
        Facts { flags: self.flags | sneaking << BLOCKING_BIT, ..self }
    }

    pub fn blocking(&self) -> bool {
        self.flags >> BLOCKING_BIT & 1 == 1
    }

    /// Answers a Molang query (without the `query.` prefix); the ones it does not know are 0.
    pub fn query(&self, name: &str) -> Value {
        if let Some(property) = name.strip_prefix("property:") {
            let sent = self.properties.iter().find(|(n, _)| &**n == property).map(|(_, v)| v.clone());
            // A cow, pig or chicken the server has sent no climate for is temperate.
            return sent.unwrap_or_else(|| if property == "minecraft:climate_variant" { Value::Text("temperate".into()) } else { Value::Num(0.0) });
        }
        Value::Num(match name {
            "variant" => self.variant as f32,
            "mark_variant" => self.mark_variant as f32,
            "skin_id" => self.skin_id as f32,
            "trade_tier" => self.trade_tier as f32,
            "color" => f32::from(self.color),
            "color2" => f32::from(self.color2),
            "health" => self.health,
            "blocking" => f32::from(u8::from(self.blocking())),
            FREEZING => self.freezing,
            CREEPER_SWELL => self.swell as f32,
            _ if name == ROW_TIME[0] => self.row_time[0],
            _ if name == ROW_TIME[1] => self.row_time[1],
            _ => FLAGS.iter().position(|(flag, _)| *flag == name).map_or(0.0, |i| (self.flags >> i & 1) as f32),
        })
    }
}
