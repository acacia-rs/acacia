//! The recipe book: `ClientboundRecipeBookAdd`/`Remove` entries, which `ServerboundPlaceRecipe` lays out by display
//! id. Only what matching a crafting recipe needs is kept; the rest of each display is skipped.

use crate::recipes::{Ingredient, read_holder_set, read_slot_display};
use crate::wire::read_len;
use crate::{Error, Wire, read_varint};

const SHAPELESS: i32 = 0;
const SHAPED: i32 = 1;
const FURNACE: i32 = 2;
const STONECUTTER: i32 = 3;
const SMITHING: i32 = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct BookRecipe {
    /// The `RecipeDisplayId` `PlaceRecipe` names.
    pub id: i32,
    pub crafting: bool,
    /// Result item id and count, when the display shows one stack.
    pub result: Option<(i32, i32)>,
    /// One ingredient per filled grid cell; empty for displays the server can't place.
    pub requirements: Vec<Ingredient>,
}

impl BookRecipe {
    /// Whether one craft's cell items (Java ids, any order) fill every requirement once. An ingredient sent as a tag
    /// accepts anything, since tags' contents aren't kept.
    pub fn takes(&self, items: &[i32]) -> bool {
        items.len() == self.requirements.len() && assign(&self.requirements, items, &mut vec![false; items.len()])
    }
}

fn assign(left: &[Ingredient], items: &[i32], used: &mut [bool]) -> bool {
    let Some((first, rest)) = left.split_first() else { return true };
    (0..items.len()).any(|i| {
        let fits = !used[i] && (matches!(first, Ingredient::Tag(_)) || first.accepts(items[i]));
        fits && {
            used[i] = true;
            let found = assign(rest, items, used);
            used[i] = false;
            found
        }
    })
}

/// `ClientboundRecipeBookAddPacket`: the entries, and whether they replace the whole book.
pub fn read_recipe_book_add(input: &mut &[u8]) -> Result<(Vec<BookRecipe>, bool), Error> {
    let count = read_len(input)?;
    let mut entries = Vec::with_capacity(count.min(4096));
    for _ in 0..count {
        entries.push(read_entry(input)?);
        u8::read(input)?;
    }
    Ok((entries, bool::read(input)?))
}

/// `ClientboundRecipeBookRemovePacket`: the display ids taken out.
pub fn read_recipe_book_remove(input: &mut &[u8]) -> Result<Vec<i32>, Error> {
    (0..read_len(input)?).map(|_| read_varint(input)).collect()
}

/// `RecipeDisplayEntry`: id, display, optional group, category, optional crafting requirements.
fn read_entry(input: &mut &[u8]) -> Result<BookRecipe, Error> {
    let id = read_varint(input)?;
    let kind = read_varint(input)?;
    let slots = |input: &mut &[u8], n: usize| (0..n).try_for_each(|_| read_slot_display(input, 0).map(drop));
    let result = match kind {
        SHAPELESS | SHAPED => {
            if kind == SHAPED {
                read_varint(input)?;
                read_varint(input)?;
            }
            let ingredients = read_len(input)?;
            slots(input, ingredients)?;
            let result = read_slot_display(input, 0)?;
            slots(input, 1)?;
            result
        }
        FURNACE => {
            slots(input, 2)?;
            let result = read_slot_display(input, 0)?;
            slots(input, 1)?;
            read_varint(input)?;
            f32::read(input)?;
            result
        }
        STONECUTTER | SMITHING => {
            slots(input, if kind == STONECUTTER { 1 } else { 3 })?;
            let result = read_slot_display(input, 0)?;
            slots(input, 1)?;
            result
        }
        _ => return Err(Error::Invalid("recipe display type")),
    };
    read_varint(input)?;
    read_varint(input)?;
    let requirements = match bool::read(input)? {
        true => (0..read_len(input)?).map(|_| read_holder_set(input)).collect::<Result<_, _>>()?,
        false => Vec::new(),
    };
    Ok(BookRecipe { id, crafting: matches!(kind, SHAPELESS | SHAPED), result, requirements })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write_varint;

    #[test]
    fn entries_read_past_displays_and_match_cell_items() {
        let mut buf = Vec::new();
        let v = |buf: &mut Vec<u8>, values: &[i32]| values.iter().for_each(|&n| write_varint(n, buf));
        v(&mut buf, &[2]);
        // shaped 1x2 sticks: id 7, two tag cells, result 4 sticks (item_stack 900 x4), empty station.
        v(&mut buf, &[7, SHAPED, 1, 2, 2, 6, 0]);
        "minecraft:planks".to_owned().write(&mut buf);
        v(&mut buf, &[6, 0]);
        "minecraft:planks".to_owned().write(&mut buf);
        v(&mut buf, &[5, 900, 4, 0, 0, 0, 0, 3]);
        true.write(&mut buf);
        v(&mut buf, &[2, 0]);
        "minecraft:planks".to_owned().write(&mut buf);
        v(&mut buf, &[0]);
        "minecraft:planks".to_owned().write(&mut buf);
        buf.push(0);
        // furnace: id 9, item 10 -> item 11, any_fuel, station empty, 200 ticks, 0.1 xp, no requirements.
        v(&mut buf, &[9, FURNACE, 4, 10, 1, 4, 11, 0, 200]);
        0.1f32.write(&mut buf);
        v(&mut buf, &[0, 2]);
        false.write(&mut buf);
        buf.push(1);
        true.write(&mut buf);
        let (entries, replace) = read_recipe_book_add(&mut &buf[..]).unwrap();
        assert!(replace);
        assert_eq!((entries[0].id, entries[0].crafting, entries[0].result), (7, true, Some((900, 4))));
        assert!(entries[0].takes(&[5, 5]) && !entries[0].takes(&[5]));
        assert_eq!((entries[1].id, entries[1].crafting, entries[1].result), (9, false, Some((11, 1))));
        let shapeless = BookRecipe { id: 1, crafting: true, result: None, requirements: vec![Ingredient::Items(vec![1]), Ingredient::Items(vec![1, 2])] };
        assert!(shapeless.takes(&[2, 1]) && !shapeless.takes(&[2, 2]));
    }
}
