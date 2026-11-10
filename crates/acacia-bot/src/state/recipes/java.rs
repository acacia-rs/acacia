//! Java's vanilla crafting recipes (acacia-registry's table) as a [`RecipeBook`]: network ids are table indices,
//! outputs are Java stacks as [`crate::state::java_inventory`] mirrors them.

use acacia_java_proto::item::ItemStack as JavaStack;
use acacia_registry::recipe::{Choice, crafting};

use super::{Ingredient, Recipe, RecipeBook, RecipeKind};
use crate::state::java_inventory::stack;

pub(super) fn book() -> RecipeBook {
    let mut book = RecipeBook::default();
    for (i, r) in crafting().iter().enumerate() {
        let kind = r.shape.map_or(RecipeKind::Shapeless, |(width, height)| RecipeKind::Shaped { width, height });
        let output = stack(Some(&JavaStack::new(i32::from(r.result.java_id()), i32::from(r.count))), &[]);
        book.push(Recipe {
            network_id: i as u32,
            id: format!("minecraft:{}", r.id),
            kind,
            block: "crafting_table".into(),
            priority: 0,
            inputs: r.inputs.iter().map(|&c| ingredient(c)).collect(),
            outputs: vec![output],
        });
    }
    book
}

fn ingredient(choice: Choice) -> Ingredient {
    match choice {
        Choice::Empty => Ingredient::Empty,
        c => Ingredient::AnyOf { items: c.items(), count: 1 },
    }
}

#[cfg(test)]
mod tests {
    use acacia_world::neutral::Item;

    use super::*;

    #[test]
    fn sticks_are_two_planks_of_any_wood_in_a_column() {
        let book = book();
        let stick = book.iter().find(|r| r.id == "minecraft:stick").unwrap();
        assert_eq!((stick.kind, stick.outputs[0].count), (RecipeKind::Shaped { width: 1, height: 2 }, 4));
        assert!(stick.inputs[0].accepts(Item::SprucePlanks.bedrock().0, 0));
        assert!(stick.fits_2x2());
    }
}
