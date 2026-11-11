#![cfg(feature = "acacia-world")]

use acacia_physics::registry_world::is_fence_or_wall;
use acacia_world::BlockRegistry;

/// The name rule the tag lookup replaced; kept to prove they agree on every vanilla Bedrock state.
fn by_name(name: &str) -> bool {
    let base = name.trim_start_matches("minecraft:");
    base.ends_with("_fence")
        || base.ends_with("_fence_gate")
        || (base.ends_with("_wall") && !["sign", "banner", "torch", "head", "skull", "fan"].iter().any(|w| base.contains(w)))
}

#[test]
fn tags_match_the_name_rule() {
    let r = BlockRegistry::vanilla();
    let differ: Vec<_> = (0..r.len() as u32)
        .map(|id| r.get(id).unwrap())
        .filter(|s| is_fence_or_wall(s) != by_name(s.name))
        .map(|s| s.name)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    // Bedrock's oak gate is plain `fence_gate`, which the name rule missed.
    assert_eq!(differ, ["minecraft:fence_gate"]);
}
