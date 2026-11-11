//! Which Java block's `animateTick` a Bedrock block state runs, with the state it reads. Names and
//! properties follow ViaBedrock's Bedrock → Java table.

use acacia_world::BlockState;
use glam::IVec3;

use super::types::Type;

/// Where a falling leaf takes its colour from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tint {
    /// `getClientLeafTintColor`: the biome's foliage.
    Foliage,
    Fixed(u32),
    Untinted,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Animate {
    None,
    /// A torch, with the step towards its wall for a wall torch (Java's `facing.getOpposite()`).
    Torch(Type, Option<IVec3>),
    RedstoneTorch(Option<IVec3>),
    Furnace { facing: IVec3, blast: bool },
    Smoker,
    Campfire { lava: bool },
    Leaves(Option<(Type, f32, Tint)>),
    NetherPortal,
    EnchantingTable,
    EndRod(IVec3),
    SporeBlossom,
    CryingObsidian,
    WetSponge,
    Mycelium,
    Candles(u8),
    CandleCake,
    BrewingStand,
    EnderChest,
    EndPortal,
    /// A `FallingBlock` and its dust colour; sand also plays its ambient sound.
    Falling { color: u32, sand: bool },
    /// Suspicious sand and gravel, dusting in their map colour.
    Brushable(u32),
    RedstoneOre,
    RedstoneWire(u8),
    Repeater { facing: IVec3, delay: u8 },
    /// A powered lever: the steps opposite its facing and its attachment.
    Lever { facing: IVec3, attached: IVec3 },
    RespawnAnchor,
    BubbleColumn { drag: bool },
    Fire { soul: bool },
    SculkSensor,
    PotentSulfur,
    WitherRose,
    DriedGhast { waterlogged: bool },
    Beehive,
    Stalactite,
    /// Blocks whose `animateTick` only rolls a sound: the bound of its `nextInt`.
    Sound(i32),
    DeadBush,
}

fn bit(state: &BlockState, key: &str) -> bool {
    state.property(key) == Some("1")
}

fn number(state: &BlockState, key: &str) -> u8 {
    state.property(key).and_then(|v| v.parse().ok()).unwrap_or(0)
}

pub fn horizontal(direction: &str) -> Option<IVec3> {
    Some(match direction {
        "north" => IVec3::NEG_Z,
        "south" => IVec3::Z,
        "west" => IVec3::NEG_X,
        "east" => IVec3::X,
        _ => return None,
    })
}

fn cardinal(state: &BlockState) -> IVec3 {
    state.property("minecraft:cardinal_direction").and_then(horizontal).unwrap_or(IVec3::Z)
}

/// Bedrock's `torch_facing_direction` names the side the flame leans to: the step to Java's wall.
fn torch_wall(state: &BlockState) -> Option<IVec3> {
    state.property("torch_facing_direction").and_then(horizontal)
}

/// `facing_direction` 0..=5 for end rods, whose horizontals Bedrock numbers the other way round.
fn end_rod_facing(state: &BlockState) -> IVec3 {
    [IVec3::NEG_Y, IVec3::Y, IVec3::Z, IVec3::NEG_Z, IVec3::X, IVec3::NEG_X][usize::from(number(state, "facing_direction")).min(5)]
}

/// Bedrock's `lever_direction` as Java's facing and attachment; floor and ceiling levers read
/// `open_bit` inverted.
fn lever(state: &BlockState) -> Option<Animate> {
    let direction = state.property("lever_direction")?;
    let (facing, attached, inverted) = match direction {
        "down_east_west" => (IVec3::X, IVec3::NEG_Y, true),
        "down_north_south" => (IVec3::Z, IVec3::NEG_Y, true),
        "up_east_west" => (IVec3::X, IVec3::Y, true),
        "up_north_south" => (IVec3::Z, IVec3::Y, true),
        d => (horizontal(d)?, horizontal(d)?, false),
    };
    (bit(state, "open_bit") != inverted).then_some(Animate::Lever { facing: -facing, attached: -attached })
}

const TINTED_LEAVES: [&str; 6] = ["oak_leaves", "birch_leaves", "jungle_leaves", "acacia_leaves", "dark_oak_leaves", "mangrove_leaves"];

pub fn classify(state: &BlockState) -> Animate {
    use Animate as A;
    let name = state.name.strip_prefix("minecraft:").unwrap_or(state.name);
    let lit = || bit(state, "lit");
    match name {
        "torch" | "underwater_torch" => A::Torch(Type::Flame, torch_wall(state)),
        n if n.starts_with("colored_torch_") => A::Torch(Type::Flame, torch_wall(state)),
        "soul_torch" => A::Torch(Type::SoulFireFlame, torch_wall(state)),
        "copper_torch" => A::Torch(Type::CopperFireFlame, torch_wall(state)),
        "redstone_torch" => A::RedstoneTorch(torch_wall(state)),
        "lit_furnace" => A::Furnace { facing: cardinal(state), blast: false },
        "lit_blast_furnace" => A::Furnace { facing: cardinal(state), blast: true },
        "lit_smoker" => A::Smoker,
        "campfire" if !bit(state, "extinguished") => A::Campfire { lava: true },
        "soul_campfire" if !bit(state, "extinguished") => A::Campfire { lava: false },
        n if TINTED_LEAVES.contains(&n) => A::Leaves(Some((Type::TintedLeaves, 0.01, Tint::Foliage))),
        "cherry_leaves" => A::Leaves(Some((Type::CherryLeaves, 0.1, Tint::Untinted))),
        "pale_oak_leaves" => A::Leaves(Some((Type::PaleOakLeaves, 0.02, Tint::Untinted))),
        "azalea_leaves" | "azalea_leaves_flowered" => A::Leaves(Some((Type::TintedLeaves, 0.01, Tint::Fixed(0x70_922D)))),
        "spruce_leaves" => A::Leaves(None),
        "portal" => A::NetherPortal,
        "enchanting_table" => A::EnchantingTable,
        "end_rod" => A::EndRod(end_rod_facing(state)),
        "spore_blossom" => A::SporeBlossom,
        "crying_obsidian" => A::CryingObsidian,
        "wet_sponge" => A::WetSponge,
        "mycelium" => A::Mycelium,
        "brewing_stand" => A::BrewingStand,
        "ender_chest" => A::EnderChest,
        "end_portal" => A::EndPortal,
        "sand" => A::Falling { color: 0xDB_D3A0, sand: true },
        "red_sand" => A::Falling { color: 0xA9_5821, sand: true },
        "gravel" => A::Falling { color: 0x80_7C7B, sand: false },
        "dragon_egg" => A::Falling { color: 0, sand: false },
        // MapColor.SAND and MapColor.STONE.
        "suspicious_sand" => A::Brushable(0xF7_E9A3),
        "suspicious_gravel" => A::Brushable(0x70_7070),
        "lit_redstone_ore" | "lit_deepslate_redstone_ore" => A::RedstoneOre,
        "redstone_wire" if number(state, "redstone_signal") > 0 => A::RedstoneWire(number(state, "redstone_signal").min(15)),
        "powered_repeater" => A::Repeater { facing: cardinal(state), delay: number(state, "repeater_delay") + 1 },
        "lever" => lever(state).unwrap_or(A::None),
        "respawn_anchor" if number(state, "respawn_anchor_charge") > 0 => A::RespawnAnchor,
        "bubble_column" => A::BubbleColumn { drag: bit(state, "drag_down") },
        "fire" => A::Fire { soul: false },
        "soul_fire" => A::Fire { soul: true },
        "sculk_sensor" | "calibrated_sculk_sensor" if number(state, "sculk_sensor_phase") == 1 => A::SculkSensor,
        "potent_sulfur" if state.property("potent_sulfur_state") != Some("dry") => A::PotentSulfur,
        "wither_rose" => A::WitherRose,
        "dried_ghast" => A::DriedGhast { waterlogged: false },
        "beehive" | "bee_nest" if number(state, "honey_level") >= 5 => A::Beehive,
        "pointed_dripstone" if bit(state, "hanging") && state.property("dripstone_thickness") == Some("tip") => A::Stalactite,
        "short_dry_grass" | "tall_dry_grass" => A::Sound(200),
        "deadbush" => A::DeadBush,
        "open_eyeblossom" => A::Sound(700),
        "pale_hanging_moss" => A::Sound(500),
        n if n.ends_with("candle_cake") && lit() => A::CandleCake,
        n if (n == "candle" || n.ends_with("_candle")) && lit() => A::Candles(number(state, "candles") + 1),
        _ => A::None,
    }
}
