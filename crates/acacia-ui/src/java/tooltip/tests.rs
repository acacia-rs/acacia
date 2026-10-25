use serde_json::json;

use super::Positioner;
use super::enchantments::level_value;
use super::fmt::decimal;

#[test]
fn decimal_format_rounds_half_even_and_trims() {
    assert_eq!(decimal(1.0), "1");
    assert_eq!(decimal(1.5), "1.5");
    assert_eq!(decimal(0.125), "0.12");
    assert_eq!(decimal(2.345), "2.35");
    assert_eq!(decimal(1.6000000953674316), "1.6");
    assert_eq!(decimal(0.001), "0");
}

#[test]
fn level_based_values() {
    assert_eq!(level_value(&json!(2.5), 3), 2.5);
    assert_eq!(level_value(&json!({"type": "minecraft:linear", "base": 0.5, "per_level_above_first": 0.25}), 3), 1.0);
    assert_eq!(level_value(&json!({"type": "minecraft:levels_squared", "added": 1.0}), 3), 10.0);
    assert_eq!(level_value(&json!({"type": "minecraft:lookup", "values": [1.0, 2.0], "fallback": 9.0}), 3), 9.0);
}

#[test]
fn default_positioner_flips_left_and_clamps_up() {
    assert_eq!(Positioner::Default.place([100, 80], 10, 40, 30, 20), [22, 28]);
    assert_eq!(Positioner::Default.place([100, 80], 90, 75, 30, 20), [48, 57]);
}

#[test]
fn below_or_above_goes_above_near_the_bottom() {
    assert_eq!(Positioner::BelowOrAbove([10, 10, 20, 10]).place([200, 100], 0, 0, 50, 20), [13, 24]);
    assert_eq!(Positioner::BelowOrAbove([10, 80, 20, 10]).place([200, 100], 0, 0, 50, 20), [13, 56]);
}
