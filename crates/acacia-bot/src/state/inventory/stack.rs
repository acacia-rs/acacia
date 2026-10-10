use acacia_client::proto::nbt::{Nbt, Value};
use acacia_client::proto::types::{
    ItemLegacy, ItemLegacyExtra, ItemStackResponsesItemContainersItemSlotsItem as ResponseSlot, ItemV4, ItemV4Extra,
};

/// An item in a slot. `network_id` 0 is air; resolve it with [`crate::state::ItemRegistry`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemStack {
    pub network_id: i32,
    pub count: u16,
    /// Damage / aux value.
    pub metadata: u32,
    pub block_runtime_id: u32,
    /// Server-assigned stack id that `ItemStackRequest`s refer to.
    pub stack_network_id: Option<i32>,
    /// `display.Name` from the item NBT (formatting codes kept).
    pub custom_name: Option<String>,
    /// `display.Lore` lines from the item NBT.
    pub lore: Vec<String>,
    /// The full item NBT, when it has any.
    pub nbt: Option<Nbt>,
}

pub(super) static EMPTY: ItemStack = ItemStack {
    network_id: 0,
    count: 0,
    metadata: 0,
    block_runtime_id: 0,
    stack_network_id: None,
    custom_name: None,
    lore: Vec::new(),
    nbt: None,
};

impl ItemStack {
    pub fn is_empty(&self) -> bool {
        self.network_id == 0 || self.count == 0
    }

    /// Whether it carries enchantments: a non-empty `ench` list in the item NBT.
    pub fn is_enchanted(&self) -> bool {
        matches!(self.nbt.as_ref().and_then(|n| n.value.get("ench")), Some(Value::List(list)) if !list.items.is_empty())
    }

    /// The dye of leather armour: `customColor` in the item NBT (ARGB), as red, green, blue.
    pub fn custom_color(&self) -> Option<[u8; 3]> {
        match self.nbt.as_ref()?.value.get("customColor")? {
            Value::Int(argb) => Some([(argb >> 16) as u8, (argb >> 8) as u8, *argb as u8]),
            _ => None,
        }
    }

    /// Applies one slot of a successful `ItemStackResponse`. The response carries no item type, so
    /// a slot that was empty stays empty here until the request sender records its prediction.
    pub(crate) fn apply_response(&mut self, slot: &ResponseSlot) {
        if slot.count == 0 {
            *self = ItemStack::default();
            return;
        }
        if self.network_id == 0 {
            return;
        }
        self.count = slot.count.into();
        self.stack_network_id = slot.item_stack_id;
        if !slot.custom_name.is_empty() {
            self.custom_name = Some(slot.custom_name.clone());
        }
    }

    fn with_nbt(mut self, nbt: Option<Nbt>) -> Self {
        let display = nbt.as_ref().and_then(|n| n.value.get("display"));
        self.custom_name = match display.and_then(|d| d.get("Name")) {
            Some(Value::String(name)) => Some(name.to_string()),
            _ => None,
        };
        self.lore = match display.and_then(|d| d.get("Lore")) {
            Some(Value::List(list)) => list
                .items
                .iter()
                .filter_map(|line| match line {
                    Value::String(s) => Some(s.to_string()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        self.nbt = nbt;
        self
    }
}

impl From<ItemV4> for ItemStack {
    fn from(item: ItemV4) -> Self {
        if item.network_id == 0 {
            return ItemStack::default();
        }
        let nbt = match item.extra {
            ItemV4Extra::Default(extra) => extra.and_then(|e| e.nbt).map(|n| n.nbt),
            ItemV4Extra::ShieldItemID(extra) => extra.and_then(|e| e.nbt).map(|n| n.nbt),
        };
        let stack = ItemStack {
            network_id: item.network_id.into(),
            count: item.count,
            metadata: item.metadata,
            block_runtime_id: item.block_runtime_id,
            stack_network_id: item.stack_id,
            ..ItemStack::default()
        };
        stack.with_nbt(nbt)
    }
}

/// Recipe outputs (`CraftingData`) carry no stack id.
impl From<&ItemLegacy> for ItemStack {
    fn from(item: &ItemLegacy) -> Self {
        if item.network_id == 0 {
            return ItemStack::default();
        }
        let nbt = match &item.extra {
            ItemLegacyExtra::Default(extra) => extra.as_ref().and_then(|e| e.nbt.as_ref()).map(|n| n.nbt.clone()),
            ItemLegacyExtra::ShieldItemID(extra) => extra.as_ref().and_then(|e| e.nbt.as_ref()).map(|n| n.nbt.clone()),
        };
        let stack = ItemStack {
            network_id: item.network_id,
            count: item.count,
            metadata: item.metadata,
            block_runtime_id: u32::try_from(item.block_runtime_id).unwrap_or(0),
            ..ItemStack::default()
        };
        stack.with_nbt(nbt)
    }
}
