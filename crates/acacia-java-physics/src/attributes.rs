//! The player's attributes that move it: bases and modifiers as `ClientboundUpdateAttributesPacket` syncs them,
//! the sprint modifier the client adds itself, and `AttributeInstance.calculateValue`.

use crate::open_map::OpenMap;

/// `LivingEntity.SPRINTING_MODIFIER_ID`; `SPEED_MODIFIER_SPRINTING` adds `0.3F` of the total.
pub const SPRINTING: &str = "minecraft:sprinting";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attribute {
    AirDragModifier,
    Bounciness,
    FrictionModifier,
    Gravity,
    JumpStrength,
    MovementEfficiency,
    MovementSpeed,
    Scale,
    SneakingSpeed,
    StepHeight,
    WaterMovementEfficiency,
}

const ALL: [Attribute; 11] = [
    Attribute::AirDragModifier,
    Attribute::Bounciness,
    Attribute::FrictionModifier,
    Attribute::Gravity,
    Attribute::JumpStrength,
    Attribute::MovementEfficiency,
    Attribute::MovementSpeed,
    Attribute::Scale,
    Attribute::SneakingSpeed,
    Attribute::StepHeight,
    Attribute::WaterMovementEfficiency,
];

impl Attribute {
    /// Its id in the built-in `attribute` registry (registration order in `Attributes`).
    pub fn from_id(id: i32) -> Option<Self> {
        Some(match id {
            0 => Self::AirDragModifier,
            9 => Self::Bounciness,
            17 => Self::FrictionModifier,
            18 => Self::Gravity,
            19 => Self::JumpStrength,
            25 => Self::MovementEfficiency,
            26 => Self::MovementSpeed,
            30 => Self::Scale,
            31 => Self::SneakingSpeed,
            33 => Self::StepHeight,
            37 => Self::WaterMovementEfficiency,
            _ => return None,
        })
    }

    pub fn default_value(self) -> f64 {
        self.defaults().0
    }

    /// A player's base (`Player.createAttributes` over `LivingEntity`'s) and the `RangedAttribute` bounds.
    fn defaults(self) -> (f64, f64, f64) {
        match self {
            Self::AirDragModifier | Self::FrictionModifier => (1.0, 0.0, 2048.0),
            Self::Bounciness => (0.0, 0.0, 1.0),
            Self::Gravity => (0.08, -1.0, 1.0),
            Self::JumpStrength => (0.42_f32 as f64, 0.0, 32.0),
            Self::MovementEfficiency | Self::WaterMovementEfficiency => (0.0, 0.0, 1.0),
            Self::MovementSpeed => (0.1_f32 as f64, 0.0, 1024.0),
            Self::Scale => (1.0, 0.0625, 16.0),
            Self::SneakingSpeed => (0.3, 0.0, 1.0),
            Self::StepHeight => (0.6, 0.0, 10.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    AddValue,
    AddMultipliedBase,
    AddMultipliedTotal,
}

impl Operation {
    pub fn from_id(id: i32) -> Option<Self> {
        Some(match id {
            0 => Self::AddValue,
            1 => Self::AddMultipliedBase,
            2 => Self::AddMultipliedTotal,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Instance {
    base: f64,
    /// `modifiersByOperation`, one map per operation in `Operation` order.
    by_operation: [OpenMap<f64>; 3],
}

impl Instance {
    fn value(&self, attribute: Attribute) -> f64 {
        let (_, min, max) = attribute.defaults();
        let [add, base_mult, total_mult] = &self.by_operation;
        let base = add.values().fold(self.base, |v, a| v + a);
        let mut result = base;
        for amount in base_mult.values() {
            result += base * amount;
        }
        for amount in total_mult.values() {
            result *= 1.0 + amount;
        }
        if result.is_nan() { min } else { result.clamp(min, max) }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attributes {
    instances: [Instance; ALL.len()],
}

impl Default for Attributes {
    fn default() -> Self {
        Self { instances: ALL.map(|a| Instance { base: a.defaults().0, by_operation: Default::default() }) }
    }
}

impl Attributes {
    fn instance(&mut self, attribute: Attribute) -> &mut Instance {
        &mut self.instances[attribute as usize]
    }

    pub fn value(&self, attribute: Attribute) -> f64 {
        self.instances[attribute as usize].value(attribute)
    }

    /// `handleUpdateAttributes` for one attribute: the base set, every modifier replaced by the packet's.
    pub fn sync(&mut self, attribute: Attribute, base: f64, modifiers: &[(String, f64, Operation)]) {
        let instance = self.instance(attribute);
        instance.base = base;
        instance.by_operation.iter_mut().for_each(OpenMap::clear);
        for (id, amount, op) in modifiers {
            instance.by_operation[*op as usize].insert(id, *amount);
        }
    }

    /// `AttributeMap.assignBaseValues`: a respawned player keeps the old bases but none of its modifiers.
    pub fn keep_bases(&mut self, old: &Attributes) {
        for (instance, old) in self.instances.iter_mut().zip(&old.instances) {
            instance.base = old.base;
        }
    }

    /// `LivingEntity.setSprinting`'s modifier: removed, then added back while sprinting.
    pub fn set_sprinting(&mut self, sprinting: bool) {
        let total = &mut self.instance(Attribute::MovementSpeed).by_operation[Operation::AddMultipliedTotal as usize];
        total.remove(SPRINTING);
        if sprinting {
            total.insert(SPRINTING, 0.3_f32 as f64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprinting_with_speed_two() {
        let mut a = Attributes::default();
        a.sync(Attribute::MovementSpeed, 0.1_f32 as f64, &[("minecraft:effect.speed".into(), 0.4_f32 as f64, Operation::AddMultipliedTotal)]);
        a.set_sprinting(true);
        let expected = [0.3_f32 as f64, 0.4_f32 as f64];
        let total = &a.instances[Attribute::MovementSpeed as usize].by_operation[2];
        let order: Vec<f64> = total.values().copied().collect();
        assert!(order == expected || order == [expected[1], expected[0]]);
        let value = order.iter().fold(0.1_f32 as f64, |v, m| v * (1.0 + m));
        assert_eq!(a.value(Attribute::MovementSpeed), value);
    }

    #[test]
    fn values_clamp_to_range() {
        let mut a = Attributes::default();
        a.sync(Attribute::Scale, 0.01, &[]);
        assert_eq!(a.value(Attribute::Scale), 0.0625);
        assert_eq!(a.value(Attribute::StepHeight), 0.6);
    }
}
