//! A bot's entity list as changes since a reader's cursor, for readers that keep a copy (the napi reader, each
//! daemon mirror). Slots, cursors and resyncs: acacia_bot::snapshot::delta.

use acacia_api::{EntityId, UniqueId};
use acacia_bot::snapshot::{self as bot, EntityInfo};
use serde::Serialize;

/// Numbers per entity in [`EntityDelta::numbers`] and in the JSON rows: feet x, y, z, velocity x, y, z, yaw,
/// pitch, health (NaN in the array and null in JSON when the server sent none).
pub const ENTITY_NUMBERS: usize = 9;

/// `[slot, ...numbers]`.
type Row<T> = (u32, T, T, T, T, T, T, T, T, Option<T>);
/// `[slot, id, uniqueId, kind, username | null]`.
type Member<'a> = (u32, EntityId, UniqueId, &'a str, Option<&'a str>);

#[derive(Serialize)]
struct Wire<'a, T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<u64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    full: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    removed: Vec<Row<T>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    added: Vec<Member<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    numbers: Vec<Row<T>>,
}

fn row<T: From<f32>>(slot: u32, e: &EntityInfo) -> Row<T> {
    let ([x, y, z], [vx, vy, vz]) = (e.position, e.velocity);
    let t = T::from;
    (slot, t(x), t(y), t(z), t(vx), t(vy), t(vz), t(e.yaw), t(e.pitch), e.health.map(t))
}

fn member(slot: u32, e: &EntityInfo) -> Member<'_> {
    (slot, EntityId(e.runtime_id), UniqueId(e.unique_id), &e.kind, e.username.as_deref())
}

/// From a reader's cursor to [`EntityDelta::version`]: apply `removed`, then `added`, then the numbers.
pub struct EntityDelta(pub(crate) bot::EntityDelta);

impl EntityDelta {
    /// The reader's next cursor.
    pub fn version(&self) -> u64 {
        self.0.list.version
    }

    /// The reader starts over: drop what is not re-added.
    pub fn full(&self) -> bool {
        self.0.full
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every slot is below this.
    pub fn slots(&self) -> u32 {
        self.0.list.slot_capacity()
    }

    /// The slot and numbers of every added or moved entity.
    pub fn numbers(&self) -> impl Iterator<Item = (u32, [f64; ENTITY_NUMBERS])> + '_ {
        let list = &self.0.list;
        self.0.added.iter().chain(&self.0.moved).map(|&i| {
            let r: Row<f64> = row(list.slots[i], &list.entities[i]);
            (r.0, [r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8, r.9.unwrap_or(f64::NAN)])
        })
    }

    /// `{removed?, added?}` (rows as in [`EntityDelta::to_json`], numbers widened to f64), or `None` when no
    /// entity came or went.
    pub fn members_json(&self) -> Option<String> {
        let wire = self.wire::<f64>(false);
        (!wire.removed.is_empty() || !wire.added.is_empty()).then(|| to_json(&wire))
    }

    /// `{version, full?, removed?, added?, numbers?}`: `removed` and `numbers` rows are `[slot, x, y, z, vx, vy,
    /// vz, yaw, pitch, health | null]` (f32), `added` rows `[slot, id, uniqueId, kind, username | null]`.
    pub fn to_json(&self) -> String {
        to_json(&self.wire::<f32>(true))
    }

    fn wire<T: From<f32>>(&self, with_numbers: bool) -> Wire<'_, T> {
        let (d, list) = (&self.0, &self.0.list);
        let changed = d.added.iter().chain(&d.moved);
        Wire {
            version: with_numbers.then_some(list.version),
            full: with_numbers && d.full,
            removed: d.removed.iter().map(|(slot, e)| row(*slot, e)).collect(),
            added: d.added.iter().map(|&i| member(list.slots[i], &list.entities[i])).collect(),
            numbers: match with_numbers {
                true => changed.map(|&i| row(list.slots[i], &list.entities[i])).collect(),
                false => Vec::new(),
            },
        }
    }
}

fn to_json(value: &impl Serialize) -> String {
    serde_json::to_string(value).expect("ids, strings and finite numbers serialize")
}
