//! Offsets of the workstation slots in the player's UI window (124), as vanilla, dragonfly and
//! Geyser number them; requests name each by its own container type plus this offset.

use acacia_client::proto::types::ContainerSlotType;

pub const ANVIL_INPUT: u8 = 1;
pub const ANVIL_MATERIAL: u8 = 2;
pub const STONECUTTER_INPUT: u8 = 3;
pub const TRADE_INGREDIENT_1: u8 = 4;
pub const TRADE_INGREDIENT_2: u8 = 5;
pub const LOOM_BANNER: u8 = 9;
pub const LOOM_DYE: u8 = 10;
pub const LOOM_PATTERN: u8 = 11;
pub const CARTOGRAPHY_INPUT: u8 = 12;
pub const CARTOGRAPHY_ADDITIONAL: u8 = 13;
pub const ENCHANTING_INPUT: u8 = 14;
pub const ENCHANTING_LAPIS: u8 = 15;
pub const GRINDSTONE_INPUT: u8 = 16;
pub const GRINDSTONE_ADDITIONAL: u8 = 17;
pub const BEACON_PAYMENT: u8 = 27;
/// First cell of the player's own 2x2 grid (28-31, row by row).
pub const CRAFTING_2X2: u8 = 28;
/// First cell of the crafting table's 3x3 grid (32-40, row by row).
pub const CRAFTING_3X3: u8 = 32;
pub const CRAFTING_3X3_LAST: u8 = 40;
/// Where craft results appear before they are moved out.
pub const CREATED_OUTPUT: u8 = 50;
pub const SMITHING_INPUT: u8 = 51;
pub const SMITHING_MATERIAL: u8 = 52;
pub const SMITHING_TEMPLATE: u8 = 53;

/// The request container type of UI offset `slot`; `None` for offsets without one (0 is the cursor).
pub(crate) fn slot_type(slot: u8) -> Option<ContainerSlotType> {
    use ContainerSlotType as T;
    Some(match slot {
        ANVIL_INPUT => T::AnvilInput,
        ANVIL_MATERIAL => T::AnvilMaterial,
        STONECUTTER_INPUT => T::StonecutterInput,
        TRADE_INGREDIENT_1 => T::Trade2Ingredient1,
        TRADE_INGREDIENT_2 => T::Trade2Ingredient2,
        LOOM_BANNER => T::LoomInput,
        LOOM_DYE => T::LoomDye,
        LOOM_PATTERN => T::LoomMaterial,
        CARTOGRAPHY_INPUT => T::CartographyInput,
        CARTOGRAPHY_ADDITIONAL => T::CartographyAdditional,
        ENCHANTING_INPUT => T::EnchantingInput,
        ENCHANTING_LAPIS => T::EnchantingLapis,
        GRINDSTONE_INPUT => T::GrindstoneInput,
        GRINDSTONE_ADDITIONAL => T::GrindstoneAdditional,
        BEACON_PAYMENT => T::BeaconPayment,
        CRAFTING_2X2..=CRAFTING_3X3_LAST => T::CraftingInput,
        CREATED_OUTPUT => T::CreativeOutput,
        SMITHING_INPUT => T::SmithingTableInput,
        SMITHING_MATERIAL => T::SmithingTableMaterial,
        SMITHING_TEMPLATE => T::SmithingTableTemplate,
        _ => return None,
    })
}
