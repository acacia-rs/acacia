//! Synced actor properties: each kind's definitions (`SyncEntityProperty`, one per kind at login)
//! and the values `AddEntity` and `SetEntityData` carry by index into them. The NBT's shape is
//! BDS's (crates/acacia-nbt/corpus/network/entity-property-*.nbt): `type` (the kind) and
//! `properties`, each with `name`, `type` (0 int, 1 float, 2 bool, 3 enum) and an enum's `enum`.

use std::collections::HashMap;

use acacia_client::proto::nbt::Value;
use acacia_client::proto::packets::SyncEntityProperty;
use acacia_client::proto::types::EntityProperties;
use acacia_client::proto::DecodeError;

/// A property's value as its definition reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Property<'a> {
    Int(i32),
    Float(f32),
    Bool(bool),
    Enum(&'a str),
}

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Int,
    Float,
    Bool,
    Enum(Vec<String>),
}

/// By entity kind, its properties in index order.
#[derive(Debug, Default)]
pub struct Definitions(HashMap<String, Vec<(String, Kind)>>);

/// An entity's property values by index, as last sent.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Values {
    ints: Vec<(u32, i32)>,
    floats: Vec<(u32, f32)>,
}

impl Values {
    /// Takes the values `p` carries; the ones it leaves out keep theirs.
    pub fn merge(&mut self, p: EntityProperties) {
        for item in p.ints {
            set(&mut self.ints, item.index, item.value);
        }
        for item in p.floats {
            set(&mut self.floats, item.index, item.value);
        }
    }
}

fn set<T>(values: &mut Vec<(u32, T)>, index: u32, value: T) {
    match values.iter_mut().find(|(i, _)| *i == index) {
        Some(slot) => slot.1 = value,
        None => values.push((index, value)),
    }
}

impl Definitions {
    pub fn apply(&mut self, p: &SyncEntityProperty) -> Result<(), DecodeError> {
        let root = p.nbt.decode()?.value;
        let Some(Value::String(kind)) = root.get("type") else { return Ok(()) };
        let Some(Value::List(list)) = root.get("properties") else { return Ok(()) };
        let defs = list.items.iter().filter_map(|d| {
            let Some(Value::String(name)) = d.get("name") else { return None };
            let kind = match d.get("type") {
                Some(Value::Int(0)) => Kind::Int,
                Some(Value::Int(1)) => Kind::Float,
                Some(Value::Int(2)) => Kind::Bool,
                Some(Value::Int(3)) => match d.get("enum") {
                    Some(Value::List(e)) => Kind::Enum(e.items.iter().filter_map(|v| if let Value::String(s) = v { Some(s.to_string()) } else { None }).collect()),
                    _ => Kind::Enum(Vec::new()),
                },
                _ => return None,
            };
            Some((name.to_string(), kind))
        });
        self.0.insert(kind.to_string(), defs.collect());
        Ok(())
    }

    /// Property `name` of an entity of `kind` holding `values`.
    pub fn value<'a>(&'a self, kind: &str, name: &str, values: &Values) -> Option<Property<'a>> {
        let index = self.0.get(kind)?.iter().position(|(n, _)| n == name)?;
        self.read(kind, index, values)
    }

    /// Every property of an entity of `kind` holding `values` that the server has sent.
    pub fn all<'a>(&'a self, kind: &str, values: &'a Values) -> impl Iterator<Item = (&'a str, Property<'a>)> {
        let defs = self.0.get_key_value(kind);
        let count = defs.map_or(0, |(_, d)| d.len());
        (0..count).filter_map(move |index| {
            let (kind, defs) = defs?;
            Some((defs[index].0.as_str(), self.read(kind, index, values)?))
        })
    }

    fn read<'a>(&'a self, kind: &str, index: usize, values: &Values) -> Option<Property<'a>> {
        let (_, def) = self.0.get(kind)?.get(index)?;
        let at = index as u32;
        let int = || values.ints.iter().find(|(i, _)| *i == at).map(|(_, v)| *v);
        Some(match def {
            Kind::Int => Property::Int(int()?),
            Kind::Bool => Property::Bool(int()? != 0),
            Kind::Enum(names) => Property::Enum(names.get(usize::try_from(int()?).ok()?)?),
            Kind::Float => Property::Float(values.floats.iter().find(|(i, _)| *i == at).map(|(_, v)| *v)?),
        })
    }
}
