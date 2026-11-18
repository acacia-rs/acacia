//! The multi-field component values: books, potions, profiles and attribute modifiers.

use super::{
    ATTRIBUTE_MODIFIERS, ComponentValue, POTION_CONTENTS, PROFILE, WRITABLE_BOOK_CONTENT, WRITTEN_BOOK_CONTENT,
};
use crate::item::codecs::{Skipped, bytes, optional, string};
use crate::text::Component;
use crate::types::{AttributeModifierOperation, Filterable};
use crate::{Error, VarInt, Wire, read_varint, write_varint};

/// `WritableBookContent`: the pages as typed (and, for a signing client, text-filtered).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WritableBook(pub Vec<Filterable<String>>);
component_value!(WritableBook, WRITABLE_BOOK_CONTENT, |input| Vec::read(input).map(Self), |v, out| v.0.write(out));

#[derive(Debug, Clone, PartialEq)]
pub struct WrittenBook {
    pub title: Filterable<String>,
    pub author: String,
    pub generation: i32,
    pub pages: Vec<Filterable<Component>>,
    /// Whether the page components are already resolved (selectors and scores substituted).
    pub resolved: bool,
}
component_value!(
    WrittenBook,
    WRITTEN_BOOK_CONTENT,
    |input| Ok(Self {
        title: Filterable::read(input)?,
        author: String::read(input)?,
        generation: read_varint(input)?,
        pages: Vec::read(input)?,
        resolved: bool::read(input)?,
    }),
    |v, out| {
        v.title.write(out);
        v.author.write(out);
        VarInt(v.generation).write(out);
        v.pages.write(out);
        v.resolved.write(out);
    }
);

/// `MobEffectInstance.Details`: the part after the effect id, with the hidden effect it replaced.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EffectDetails {
    pub amplifier: i32,
    pub duration: i32,
    pub ambient: bool,
    pub visible: bool,
    pub show_icon: bool,
    pub hidden: Option<Box<EffectDetails>>,
}

impl Wire for EffectDetails {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let (amplifier, duration) = (read_varint(input)?, read_varint(input)?);
        let (ambient, visible, show_icon) = (bool::read(input)?, bool::read(input)?, bool::read(input)?);
        let hidden = if bool::read(input)? { Some(Box::new(Self::read(input)?)) } else { None };
        Ok(Self { amplifier, duration, ambient, visible, show_icon, hidden })
    }

    fn write(&self, out: &mut Vec<u8>) {
        write_varint(self.amplifier, out);
        write_varint(self.duration, out);
        [self.ambient, self.visible, self.show_icon].iter().for_each(|b| b.write(out));
        self.hidden.is_some().write(out);
        if let Some(hidden) = &self.hidden {
            hidden.write(out);
        }
    }
}

/// `MobEffectInstance.STREAM_CODEC`: the `mob_effect` registry id, then its details.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MobEffectInstance {
    pub effect: i32,
    pub details: EffectDetails,
}

impl Wire for MobEffectInstance {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { effect: read_varint(input)?, details: EffectDetails::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        write_varint(self.effect, out);
        self.details.write(out);
    }
}

/// `PotionContents`: the `potion` registry id, a custom ARGB colour, extra effects and a name key.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PotionContents {
    pub potion: Option<i32>,
    pub custom_color: Option<i32>,
    pub effects: Vec<MobEffectInstance>,
    pub custom_name: Option<String>,
}
component_value!(
    PotionContents,
    POTION_CONTENTS,
    |input| Ok(Self {
        potion: Option::<VarInt>::read(input)?.map(|v| v.0),
        custom_color: if bool::read(input)? { Some(i32::read(input)?) } else { None },
        effects: Vec::read(input)?,
        custom_name: if bool::read(input)? { Some(String::read(input)?) } else { None },
    }),
    |v, out| {
        v.potion.map(VarInt).write(out);
        v.custom_color.is_some().write(out);
        if let Some(color) = v.custom_color {
            color.write(out);
        }
        v.effects.write(out);
        v.custom_name.is_some().write(out);
        if let Some(name) = &v.custom_name {
            name.write(out);
        }
    }
);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProfileProperty {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

impl Wire for ProfileProperty {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let (name, value) = (String::read(input)?, String::read(input)?);
        Ok(Self { name, value, signature: if bool::read(input)? { Some(String::read(input)?) } else { None } })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.name.write(out);
        self.value.write(out);
        self.signature.is_some().write(out);
        if let Some(sig) = &self.signature {
            sig.write(out);
        }
    }
}

/// `ResolvableProfile` in its full form (uuid, name, properties) with no skin patch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Profile {
    pub uuid: u128,
    pub name: String,
    pub properties: Vec<ProfileProperty>,
}
component_value!(
    Profile,
    PROFILE,
    |input| {
        if !bool::read(input)? {
            return Err(Error::Invalid("partial resolvable profile"));
        }
        let profile = Self { uuid: u128::read(input)?, name: String::read(input)?, properties: Vec::read(input)? };
        let patch: Skipped = (0..3).try_for_each(|_| optional(input, string)).and_then(|()| optional(input, bytes::<1>));
        patch.map(|()| profile)
    },
    |v, out| {
        true.write(out);
        v.uuid.write(out);
        v.name.write(out);
        v.properties.write(out);
        out.extend([0; 4]);
    }
);

/// `ItemAttributeModifiers.Display`.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AttributeDisplay {
    #[default]
    Default,
    Hidden,
    Override(Component),
}

/// `ItemAttributeModifiers.Entry`: an `attribute` registry id, the modifier, and the `EquipmentSlotGroup` id.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeEntry {
    pub attribute: i32,
    pub id: String,
    pub amount: f64,
    pub operation: AttributeModifierOperation,
    pub slot_group: i32,
    pub display: AttributeDisplay,
}

impl Wire for AttributeEntry {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let attribute = read_varint(input)?;
        let (id, amount, operation, slot_group) = (String::read(input)?, f64::read(input)?, AttributeModifierOperation::read(input)?, read_varint(input)?);
        let display = match read_varint(input)? {
            0 => AttributeDisplay::Default,
            1 => AttributeDisplay::Hidden,
            2 => AttributeDisplay::Override(Component::read(input)?),
            _ => return Err(Error::Invalid("attribute modifier display")),
        };
        Ok(Self { attribute, id, amount, operation, slot_group, display })
    }

    fn write(&self, out: &mut Vec<u8>) {
        write_varint(self.attribute, out);
        self.id.write(out);
        self.amount.write(out);
        self.operation.write(out);
        write_varint(self.slot_group, out);
        match &self.display {
            AttributeDisplay::Default => write_varint(0, out),
            AttributeDisplay::Hidden => write_varint(1, out),
            AttributeDisplay::Override(text) => {
                write_varint(2, out);
                text.write(out);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AttributeModifiers(pub Vec<AttributeEntry>);
component_value!(AttributeModifiers, ATTRIBUTE_MODIFIERS, |input| Vec::read(input).map(Self), |v, out| v.0.write(out));
