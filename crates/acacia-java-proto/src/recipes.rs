//! `ClientboundUpdateRecipesPacket` (play): the stonecutter's recipes in the server's order, which is also the order
//! of its menu buttons. The recipe property sets before them are skipped.

use crate::item::Components;
use crate::{Error, Wire, nbt, read_varint};

/// A recipe's input: a list of item ids, or an item tag (vanilla stonecutting uses none).
#[derive(Debug, Clone, PartialEq)]
pub enum Ingredient {
    Items(Vec<i32>),
    Tag(String),
}

impl Ingredient {
    pub fn accepts(&self, item: i32) -> bool {
        matches!(self, Ingredient::Items(items) if items.contains(&item))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StonecutterRecipe {
    pub input: Ingredient,
    /// Result item id and count, from the recipe's display; `None` for a display that shows no single stack.
    pub result: Option<(i32, i32)>,
}

const MAX_DEPTH: u32 = 64;
const SLOT_DISPLAY_ITEM: i32 = 4;
const SLOT_DISPLAY_ITEM_STACK: i32 = 5;

/// The packet's stonecutter list.
pub fn read_update_recipes(input: &mut &[u8]) -> Result<Vec<StonecutterRecipe>, Error> {
    for _ in 0..read_varint(input)? {
        String::read(input)?;
        for _ in 0..read_varint(input)? {
            read_varint(input)?;
        }
    }
    let count = read_varint(input)?;
    let mut recipes = Vec::with_capacity(count.clamp(0, 1024) as usize);
    for _ in 0..count {
        let input_set = read_holder_set(input)?;
        recipes.push(StonecutterRecipe { input: input_set, result: read_slot_display(input, 0)? });
    }
    Ok(recipes)
}

/// `ByteBufCodecs.holderSet`: 0 then a tag, else the item count plus one, then the ids.
pub(crate) fn read_holder_set(input: &mut &[u8]) -> Result<Ingredient, Error> {
    match read_varint(input)? {
        0 => Ok(Ingredient::Tag(String::read(input)?)),
        n => (1..n).map(|_| read_varint(input)).collect::<Result<_, _>>().map(Ingredient::Items),
    }
}

/// `SlotDisplay.STREAM_CODEC`: a type id from `SlotDisplays.bootstrap`'s order, then its fields. Returns the stack an
/// `item` or `item_stack` display shows.
pub(crate) fn read_slot_display(input: &mut &[u8], depth: u32) -> Result<Option<(i32, i32)>, Error> {
    if depth >= MAX_DEPTH {
        return Err(Error::TooDeep(MAX_DEPTH));
    }
    let nested = |input: &mut &[u8]| read_slot_display(input, depth + 1).map(drop);
    match read_varint(input)? {
        0 | 1 => {}
        2 => nested(input)?,
        3 => {
            nested(input)?;
            read_varint(input)?;
        }
        SLOT_DISPLAY_ITEM => return Ok(Some((read_varint(input)?, 1))),
        SLOT_DISPLAY_ITEM_STACK => {
            let (item, count) = (read_varint(input)?, read_varint(input)?);
            Components::read(input)?;
            return Ok(Some((item, count)));
        }
        6 => drop(read_holder_set(input)?),
        7 | 9 => (0..2).try_for_each(|_| nested(input))?,
        8 => {
            (0..2).try_for_each(|_| nested(input))?;
            // `Holder<TrimPattern>`: 0 then the pattern inline (asset id, description, decal), else its id plus one.
            if read_varint(input)? == 0 {
                String::read(input)?;
                nbt::skip_tag(input)?;
                bool::read(input)?;
            }
        }
        10 => (0..read_varint(input)?).try_for_each(|_| nested(input))?,
        _ => return Err(Error::Invalid("slot display type")),
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write_varint;

    #[test]
    fn stonecutter_entries_keep_order_past_item_sets_and_nested_displays() {
        let mut buf = Vec::new();
        write_varint(1, &mut buf);
        "minecraft:furnace_input".to_owned().write(&mut buf);
        [2, 7, 9].iter().for_each(|&v| write_varint(v, &mut buf));
        write_varint(2, &mut buf);
        // andesite (6) -> 2 andesite slabs (700), no components.
        [2, 6, SLOT_DISPLAY_ITEM_STACK, 700, 2, 0, 0].iter().for_each(|&v| write_varint(v, &mut buf));
        // a tag input with a composite display of one item.
        write_varint(0, &mut buf);
        "minecraft:stones".to_owned().write(&mut buf);
        [10, 1, SLOT_DISPLAY_ITEM, 5].iter().for_each(|&v| write_varint(v, &mut buf));
        let recipes = read_update_recipes(&mut &buf[..]).unwrap();
        assert_eq!(recipes[0], StonecutterRecipe { input: Ingredient::Items(vec![6]), result: Some((700, 2)) });
        assert_eq!((recipes[1].input.accepts(6), recipes[1].result), (false, None));
    }
}
