//! The local player's abilities from `UpdateAbilities` (StartGame carries none since protocol 1.19.10).

use acacia_client::proto::packets::UpdateAbilities;
use acacia_client::proto::types::{AbilityLayers, AbilityLayersType, AbilitySet};

/// Movement abilities, resolved over the ability layers: a layer above the base one (spectator, commands,
/// editor) overrides the abilities it allows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Abilities {
    pub may_fly: bool,
    pub flying: bool,
    pub no_clip: bool,
    pub fly_speed: f32,
    pub vertical_fly_speed: f32,
    pub walk_speed: f32,
}

impl Default for Abilities {
    fn default() -> Self {
        Self { may_fly: false, flying: false, no_clip: false, fly_speed: 0.05, vertical_fly_speed: 1.0, walk_speed: 0.1 }
    }
}

impl Abilities {
    pub(crate) fn apply(&mut self, p: &UpdateAbilities) {
        let mut layers: Vec<&AbilityLayers> = p.abilities.iter().filter(|l| l.r#type != AbilityLayersType::Cache).collect();
        layers.sort_by_key(|l| l.r#type != AbilityLayersType::Base);
        for layer in layers {
            let allows = |a: AbilitySet| layer.allowed.contains(a);
            let enabled = |a: AbilitySet| layer.enabled.contains(a);
            for (ability, field) in [
                (AbilitySet::MAY_FLY, &mut self.may_fly),
                (AbilitySet::FLYING, &mut self.flying),
                (AbilitySet::NO_CLIP, &mut self.no_clip),
            ] {
                if allows(ability) {
                    *field = enabled(ability);
                }
            }
            for (ability, value, field) in [
                (AbilitySet::FLY_SPEED, layer.fly_speed, &mut self.fly_speed),
                (AbilitySet::VERTICAL_FLY_SPEED, layer.vertical_fly_speed, &mut self.vertical_fly_speed),
                (AbilitySet::WALK_SPEED, layer.walk_speed, &mut self.walk_speed),
            ] {
                if allows(ability) {
                    *field = value;
                }
            }
        }
    }
}
