//! The creative inventory's items (`CreativeContent`). Kept raw and decoded the first time they
//! are asked for, like the recipes.

use std::sync::OnceLock;

use acacia_client::proto::packets::{CreativeContent, CreativeContentGroupsItemCategory as Category};
use acacia_client::proto::{Packet, RawPacket};

use super::ItemStack;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CreativeTab {
    Construction,
    Nature,
    Equipment,
    Items,
}

/// One entry; `entry_id` is what a `CraftCreative` action names.
#[derive(Debug, Clone, PartialEq)]
pub struct CreativeItem {
    pub entry_id: u32,
    /// One of the item.
    pub stack: ItemStack,
    pub tab: CreativeTab,
}

#[derive(Debug, Default)]
pub struct Creative {
    packet: Option<RawPacket>,
    items: OnceLock<Vec<CreativeItem>>,
}

impl Creative {
    pub const PACKETS: &'static [u32] = &[CreativeContent::ID];

    pub fn apply(&mut self, packet: &RawPacket) {
        self.packet = Some(packet.clone());
        self.items = OnceLock::new();
    }

    /// Every item in the server's order; entries of groups outside the four tabs (command-only
    /// items) are left out.
    pub fn items(&self) -> &[CreativeItem] {
        self.items.get_or_init(|| {
            let content = match self.packet.as_ref().map(|p| p.decode::<CreativeContent>()) {
                Some(Ok(content)) => content,
                Some(Err(e)) => {
                    tracing::debug!(error = %e, "cannot decode CreativeContent");
                    return Vec::new();
                }
                None => return Vec::new(),
            };
            let entry = |item: &acacia_client::proto::packets::CreativeContentItemsItem| {
                let tab = match content.groups.get(item.group_index as usize)?.category {
                    Category::Construction => CreativeTab::Construction,
                    Category::Nature => CreativeTab::Nature,
                    Category::Equipment => CreativeTab::Equipment,
                    Category::Items => CreativeTab::Items,
                    _ => return None,
                };
                let stack = ItemStack { count: 1, ..ItemStack::from(&item.item) };
                (!stack.is_empty()).then_some(CreativeItem { entry_id: item.entry_id, stack, tab })
            };
            content.items.iter().filter_map(entry).collect()
        })
    }
}
