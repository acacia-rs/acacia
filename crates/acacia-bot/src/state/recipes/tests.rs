use super::*;
use crate::state::queries::test_support::{crafting_data, fixtures, ingredient, legacy_item, raw, shaped, shapeless};

const STICK: i32 = 320;
const TORCH: i32 = 50;

fn sample() -> CraftingData {
    let stick = shaped(7, (1, 2), vec![ingredient("#minecraft:planks", 1), ingredient("#minecraft:planks", 1)], legacy_item(STICK, 4));
    let torch = shapeless(9, "crafting_table", vec![ingredient("minecraft:coal", 1), ingredient("minecraft:stick", 1)], legacy_item(TORCH, 4));
    let slab = shapeless(11, "stonecutter", vec![ingredient("minecraft:stone", 1)], legacy_item(-166, 2));
    crafting_data(vec![stick], vec![torch, slab], true)
}

#[test]
fn decodes_lazily_on_first_lookup() {
    let mut recipes = Recipes::default();
    assert!(!recipes.received());
    recipes.apply(&raw(&sample()));
    assert!(recipes.received());
    assert!(recipes.book.get().is_none(), "nothing is decoded before the first lookup");

    let book = recipes.book();
    assert_eq!(book.len(), 3);
    let stick = book.get(7).unwrap();
    assert_eq!(stick.kind, RecipeKind::Shaped { width: 1, height: 2 });
    assert_eq!(stick.block, "crafting_table");
    assert_eq!(stick.inputs[0], Ingredient::Tag { tag: "minecraft:planks".into(), count: 1 });
    assert_eq!((stick.outputs[0].network_id, stick.outputs[0].count), (STICK, 4));
    assert!(stick.fits_2x2());
    assert_eq!(book.producing(TORCH).map(|r| r.network_id).collect::<Vec<_>>(), [9]);
    assert_eq!(book.get(11).unwrap().block, "stonecutter");
}

#[test]
fn later_packets_add_or_replace() {
    let mut recipes = Recipes::default();
    recipes.apply(&raw(&sample()));
    assert_eq!(recipes.book().len(), 3);
    let extra = shapeless(20, "crafting_table", vec![ingredient("minecraft:stone", 1)], legacy_item(77, 1));
    recipes.apply(&raw(&crafting_data(vec![], vec![extra.clone()], false)));
    assert_eq!(recipes.book().len(), 4, "a packet without clear_recipes adds");
    recipes.apply(&raw(&crafting_data(vec![], vec![extra], true)));
    assert_eq!(recipes.book().len(), 1, "clear_recipes drops what came before");
}

#[test]
fn ingredients_match_items_tags_and_metadata() {
    let any = Ingredient::Item { name: "minecraft:coal".into(), metadata: None, count: 1 };
    assert!(any.accepts("minecraft:coal", 3));
    let exact = Ingredient::Item { name: "minecraft:coal".into(), metadata: Some(0), count: 1 };
    assert!(exact.accepts("minecraft:coal", 0) && !exact.accepts("minecraft:coal", 1));
    let tag = Ingredient::Tag { tag: "minecraft:planks".into(), count: 1 };
    assert!(tag.accepts("minecraft:oak_planks", 0) && !tag.accepts("minecraft:oak_log", 0));
    let molang = Ingredient::Molang { expression: "query.any_tag('minecraft:coals')".into(), version: 12, count: 1 };
    assert!(molang.accepts("minecraft:charcoal", 0));
    assert!(Ingredient::Empty.is_empty() && !Ingredient::Empty.accepts("minecraft:air", 0));
}

#[test]
fn blank_cells_decode_as_empty() {
    let data = crafting_data(vec![shaped(3, (2, 1), vec![ingredient("minecraft:stick", 1), ingredient("", 0)], legacy_item(1, 1))], vec![], true);
    let mut recipes = Recipes::default();
    recipes.apply(&raw(&data));
    assert_eq!(recipes.book().get(3).unwrap().inputs[1], Ingredient::Empty);
}

#[test]
fn decodes_fixtures() {
    let mut recipes = Recipes::default();
    for packet in fixtures::<CraftingData>() {
        recipes.apply(&packet);
    }
    let _ = recipes.book().len();
}
