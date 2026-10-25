//! Bedrock entity state to Java's render state: the one place the Molang queries the viewer
//! answers (README "Entities") become the fields `setupAnim` reads. Where Java's renderer derives
//! a field (`getArmPose`, `Sheep.getHeadEatAngleScale`), the rule it follows is here too.

use super::anim::{Family, sheep_eat_scales};
use super::state::{Field, RenderState};
use crate::entity::molang::Value;

/// Ticks per second of `life_time`.
const TICKS: f32 = 20.0;

/// For an entity drawn by a model of `family`.
pub fn render_state(family: Family, query: &dyn Fn(&str) -> Value) -> RenderState {
    let num = |name: &str| query(name).num();
    let flag = |name: &str| query(name).truthy();
    let held = match query("get_equipped_item_name:main_hand") {
        Value::Text(name) if !name.is_empty() => Some(name),
        _ => None,
    };
    let mut s = RenderState::default();
    let baby = flag("is_baby");
    let aggressive = flag("is_angry");
    s.set("walkAnimationPos", Field::Num(num("modified_distance_moved")))
        .set("walkAnimationSpeed", Field::Num(num("modified_move_speed").min(1.0)))
        .set("ageInTicks", Field::Num(num("life_time") * TICKS))
        .set("yRot", Field::Num(num("target_y_rotation")))
        .set("xRot", Field::Num(num("target_x_rotation")))
        .set("isBaby", Field::Flag(baby))
        .set("ageScale", Field::Num(if baby { 0.5 } else { 1.0 }))
        .set("isCrouching", Field::Flag(flag("is_sneaking")))
        .set("isPassenger", Field::Flag(flag("is_riding")))
        .set("isAggressive", Field::Flag(aggressive));
    if let Some(item) = &held {
        // Bedrock mobs hold in the right hand (mainArm RIGHT).
        s.set("rightHandItemStack", Field::Text(format!("minecraft:{item}")));
    }
    // getArmPose: HumanoidMobRenderer's EMPTY, AbstractSkeletonRenderer's bow and
    // DrownedRenderer's trident, in the main (right) arm of an aggressive mob.
    // TODO: SPEAR for a held spear or a stab swing.
    let raised = match (family, held.as_deref()) {
        (Family::Skeleton, Some("bow")) if aggressive => Some("BOW_AND_ARROW"),
        (Family::Drowned, Some("trident")) if aggressive => Some("THROW_TRIDENT"),
        _ => None,
    };
    if let Some(pose) = raised {
        s.set("rightArmPose", Field::Text(pose.into()));
    }
    s.set("isHoldingBow", Field::Flag(held.as_deref() == Some("bow")));
    if family == Family::Sheep {
        // TODO: the grazing timer; Bedrock sends no eating ticks the viewer reads yet.
        let (position, angle) = sheep_eat_scales(0, 0.0, num("target_x_rotation"));
        s.set("headEatPositionScale", Field::Num(position)).set("headEatAngleScale", Field::Num(angle));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_angry_skeleton_draws_its_bow() {
        let query = |name: &str| match name {
            "is_angry" => Value::Num(1.0),
            "get_equipped_item_name:main_hand" => Value::Text("bow".into()),
            "life_time" => Value::Num(2.0),
            _ => Value::Num(0.0),
        };
        let s = render_state(Family::Skeleton, &query);
        assert_eq!((s.text("rightArmPose"), s.flag("isHoldingBow"), s.age()), (Some("BOW_AND_ARROW"), true, 40.0));
        assert_eq!(render_state(Family::Zombie, &query).text("rightArmPose"), None);
    }
}
