//! Java's vanilla crafting recipes (shaped and shapeless), for servers that send no recipe data a bot could plan
//! with: `crafting().iter().filter(|r| r.result == Item::Stick)`.

use crate::item::Item;
use crate::tag::ItemTag;

/// One grid cell's accepted items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Empty,
    Item(Item),
    Items(&'static [Item]),
    Tag(ItemTag),
}

impl Choice {
    pub fn accepts(self, item: Item) -> bool {
        match self {
            Choice::Empty => false,
            Choice::Item(i) => i == item,
            Choice::Items(items) => items.contains(&item),
            Choice::Tag(tag) => tag.contains(item),
        }
    }

    pub fn items(self) -> Vec<Item> {
        match self {
            Choice::Empty => Vec::new(),
            Choice::Item(i) => vec![i],
            Choice::Items(items) => items.to_vec(),
            Choice::Tag(tag) => tag.items().collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftingRecipe {
    /// The recipe's id without `minecraft:`.
    pub id: &'static str,
    /// Shaped recipes' `(width, height)`; `inputs` then holds the cells row by row, blanks as [`Choice::Empty`].
    pub shape: Option<(u8, u8)>,
    pub inputs: &'static [Choice],
    pub result: Item,
    pub count: u8,
}

pub fn crafting() -> &'static [CraftingRecipe] {
    crate::generated::recipes::CRAFTING
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticks_take_two_planks_of_any_wood() {
        let stick = crafting().iter().find(|r| r.id == "stick").unwrap();
        assert_eq!((stick.shape, stick.result, stick.count), (Some((1, 2)), Item::Stick, 4));
        assert!(stick.inputs.iter().all(|c| c.accepts(Item::BirchPlanks)));
        let torch = crafting().iter().find(|r| r.id == "torch").unwrap();
        assert!(torch.inputs[0].accepts(Item::Charcoal) && !torch.inputs[0].accepts(Item::Stick));
    }
}
