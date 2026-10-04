use acacia_client::proto::types::{
    MetadataDictionaryItem, MetadataDictionaryItemKey as Key, MetadataDictionaryItemValue as Value,
    MetadataDictionaryItemValueDefault as Plain, MetadataFlags1,
};

/// The synced actor data a viewer needs to pick an entity's look; everything else is dropped.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityMeta {
    pub flags: MetadataFlags1,
    /// Meaning depends on the kind: cat or horse breed, villager profession, slime size...
    pub variant: i32,
    pub mark_variant: i32,
    /// Villager skin tone.
    pub skin_id: i32,
    pub trade_tier: i32,
    /// Dye colour index (sheep wool, collars, shulkers).
    pub color: i8,
    pub scale: f32,
    pub nametag: String,
}

impl Default for EntityMeta {
    fn default() -> Self {
        Self { flags: MetadataFlags1::default(), variant: 0, mark_variant: 0, skin_id: 0, trade_tier: 0, color: 0, scale: 1.0, nametag: String::new() }
    }
}

impl EntityMeta {
    pub fn from_items(items: Vec<MetadataDictionaryItem>) -> Self {
        let mut meta = Self::default();
        meta.apply(items);
        meta
    }

    /// Metadata packets carry only the keys that changed.
    pub fn apply(&mut self, items: Vec<MetadataDictionaryItem>) {
        for item in items {
            match (item.key, item.value) {
                (_, Value::Flags(f)) => self.flags = f,
                (Key::Variant, Value::Default(Plain::Int(v))) => self.variant = v,
                (Key::MarkVariant, Value::Default(Plain::Int(v))) => self.mark_variant = v,
                (Key::SkinId, Value::Default(Plain::Int(v))) => self.skin_id = v,
                (Key::TradeTier, Value::Default(Plain::Int(v))) => self.trade_tier = v,
                (Key::Color,Value::Default(Plain::Byte(v))) => self.color = v,
                (Key::Scale, Value::Default(Plain::Float(v))) => self.scale = v,
                (Key::Nametag, Value::Default(Plain::String(v))) => self.nametag = v,
                _ => {}
            }
        }
    }

    pub fn is_baby(&self) -> bool {
        self.flags.contains(MetadataFlags1::BABY)
    }

    pub fn is_invisible(&self) -> bool {
        self.flags.contains(MetadataFlags1::INVISIBLE)
    }

    pub fn is_sneaking(&self) -> bool {
        self.flags.contains(MetadataFlags1::SNEAKING)
    }
}
