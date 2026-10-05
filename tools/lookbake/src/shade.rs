//! Which blocks darken the ambient occlusion around them: Java's `BlockState.getShadeBrightness`,
//! 0.2 for a block whose collision fills its cell unless the block says otherwise.

use crate::mapping::JavaState;

/// Full blocks that stay bright (`TransparentBlock` and its kin), by a part of their name.
const BRIGHT: &[&str] = &["glass", "copper_grate", "barrier"];
/// Blocks whose collision fills the cell only in Bedrock.
const SMALLER_IN_JAVA: &[&str] = &["azalea", "flowering_azalea", "dragon_egg", "stonecutter"];
/// Blocks that darken though their collision is short of full.
const DARK: &[&str] = &["soul_sand", "mud"];

/// `full_collision` is of the Bedrock state: but for [`SMALLER_IN_JAVA`], the editions' collision
/// boxes agree on what is full.
pub fn darkens(java: &JavaState, full_collision: bool) -> bool {
    let name = java.name.as_str();
    let full = (full_collision && !SMALLER_IN_JAVA.contains(&name)) || (name == "snow" && java.property("layers") == Some("8"));
    DARK.contains(&name) || (full && !BRIGHT.iter().any(|part| name.contains(part)))
}

#[test]
fn full_blocks_darken_but_glass() {
    let state = |name: &str| JavaState { name: name.to_owned(), properties: Vec::new() };
    assert!(darkens(&state("oak_leaves"), true));
    assert!(!darkens(&state("oak_slab"), false));
    assert!(!darkens(&state("red_stained_glass"), true));
    assert!(darkens(&state("soul_sand"), false));
}
