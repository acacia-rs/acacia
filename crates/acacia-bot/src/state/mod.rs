//! Game state rebuilt from server packets. Each tracker owns one concern and declares the packet
//! IDs it consumes; [`GameState::apply`] routes packets to them.

mod block_entities;
mod containers;
mod entities;
mod entity_meta;
mod fishing;
mod inventory;
mod items;
mod player;
mod player_list;
pub(crate) mod queries;
mod recipes;
mod riding;
mod scoreboard;
mod signs;
mod skins;
mod stations;

pub use block_entities::{BlockEntities, BlockEntityTracking};
pub use containers::{Container, Containers};
pub use entities::{Entities, Entity, ITEM_KIND, PLAYER_EYE_HEIGHT, PLAYER_KIND};
pub use entity_meta::EntityMeta;
pub use skins::{PlayerSkin, SkinTexture, Skins};
pub use fishing::{Fishing, FishingHook, FISHING_HOOK_KIND};
pub use riding::{Pose, Riding, Vehicle};
pub use inventory::{Inventory, ItemStack};
pub use items::ItemRegistry;
pub use recipes::{Ingredient, Recipe, RecipeBook, RecipeKind, Recipes};
pub use stations::{EnchantOption, Stations, TradeItem, TradeOffer, TradeWindow};
pub use player::PlayerState;
pub use player_list::{PlayerList, PlayerListEntry};
pub(crate) use player_list::PlayerChange;
pub use scoreboard::{Objective, ScoreEntry, Scoreboard, SortOrder, SLOT_BELOW_NAME, SLOT_LIST, SLOT_SIDEBAR};
pub use signs::{SignEditor, Signs};

use acacia_client::proto::packets::SetEntityData;
use acacia_client::proto::{DecodeError, Packet, RawPacket};

use crate::forms::Forms;

/// Which optional trackers run (see [`crate::BotConfig`]); the rest always run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Trackers {
    pub entities: bool,
    /// Skin textures of the listed players ([`Skins`]).
    pub skins: bool,
    /// Block entities kept from chunk data (signs from `BlockActorData` are always kept).
    pub block_entities: BlockEntityTracking,
}

impl Trackers {
    pub fn packet_ids(&self) -> Vec<u32> {
        let mut ids = Vec::new();
        ids.extend_from_slice(PlayerState::PACKETS);
        ids.push(SetEntityData::ID);
        ids.extend_from_slice(ItemRegistry::PACKETS);
        ids.extend_from_slice(Recipes::PACKETS);
        ids.extend_from_slice(Stations::PACKETS);
        ids.extend_from_slice(Inventory::PACKETS);
        ids.extend_from_slice(Containers::PACKETS);
        ids.extend_from_slice(Scoreboard::PACKETS);
        ids.extend_from_slice(PlayerList::PACKETS);
        ids.extend_from_slice(Forms::PACKETS);
        ids.extend_from_slice(Signs::PACKETS);
        ids.extend_from_slice(self.block_entities.packets());
        ids.extend_from_slice(Riding::PACKETS);
        ids.extend_from_slice(Fishing::PACKETS);
        if self.entities {
            ids.extend_from_slice(Entities::PACKETS);
        }
        if self.skins {
            ids.extend_from_slice(Skins::PACKETS);
        }
        ids
    }
}

/// Identity of the local player, needed to tell its packets apart from other entities'.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Me {
    pub runtime_entity_id: u64,
    pub unique_entity_id: i64,
}

#[derive(Debug, Default)]
pub struct GameState {
    trackers: Trackers,
    pub player: PlayerState,
    pub items: ItemRegistry,
    pub recipes: Recipes,
    /// Enchanting options and trade offers of the open workstation.
    pub stations: Stations,
    pub inventory: Inventory,
    pub containers: Containers,
    pub scoreboard: Scoreboard,
    pub player_list: PlayerList,
    pub entities: Entities,
    pub skins: Skins,
    pub forms: Forms,
    pub signs: Signs,
    pub block_entities: BlockEntities,
    pub riding: Riding,
    pub fishing: Fishing,
}

impl GameState {
    pub(crate) fn new(trackers: Trackers, runtime_entity_id: u64) -> Self {
        let mut state = Self { trackers, block_entities: BlockEntities::new(trackers.block_entities), ..Self::default() };
        state.player.runtime_entity_id = runtime_entity_id;
        state
    }

    pub fn me(&self) -> Me {
        Me { runtime_entity_id: self.player.runtime_entity_id, unique_entity_id: self.player.unique_entity_id }
    }

    /// Routes one packet to every tracker that consumes its ID.
    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        let id = packet.id;
        if PlayerState::PACKETS.contains(&id) {
            self.player.apply(packet)?;
        }
        if id == SetEntityData::ID {
            self.player.apply_entity_data(packet)?;
        }
        let me = self.me();
        if ItemRegistry::PACKETS.contains(&id) {
            self.items.apply(packet)?;
        }
        if Recipes::PACKETS.contains(&id) {
            self.recipes.apply(packet);
        }
        if Stations::PACKETS.contains(&id) {
            self.stations.apply(packet)?;
        }
        if Inventory::PACKETS.contains(&id) {
            self.inventory.apply(packet, &me)?;
        }
        if Containers::PACKETS.contains(&id) {
            self.containers.apply(packet, &me)?;
        }
        if Scoreboard::PACKETS.contains(&id) {
            self.scoreboard.apply(packet)?;
        }
        if PlayerList::PACKETS.contains(&id) {
            self.player_list.apply(packet)?;
        }
        if Riding::PACKETS.contains(&id) {
            self.riding.apply(packet, &me)?;
        }
        if Fishing::PACKETS.contains(&id) {
            self.fishing.apply(packet, &me)?;
        }
        if Forms::PACKETS.contains(&id) {
            self.forms.apply(packet)?;
        }
        if Signs::PACKETS.contains(&id) {
            self.signs.apply(packet)?;
        }
        if self.trackers.block_entities.packets().contains(&id) {
            self.block_entities.apply(packet)?;
        }
        if self.trackers.entities && Entities::PACKETS.contains(&id) {
            self.entities.apply(packet, &me)?;
        }
        if self.trackers.skins && Skins::PACKETS.contains(&id) {
            self.skins.apply(packet)?;
        }
        Ok(())
    }
}
