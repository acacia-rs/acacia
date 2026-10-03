use acacia_client::proto::packets::CraftingData;
use acacia_client::proto::types::{
    ItemLegacy, RecipeIngredient, RecipeIngredientContent as C, RecipeIngredientContentValidContent as V, ShapedRecipe,
    ShapelessRecipe,
};

use super::{Ingredient, Recipe, RecipeBook, RecipeKind};
use crate::state::ItemStack;

/// Aux value meaning "any metadata".
const ANY_METADATA: i32 = 32767;

pub(super) fn add(book: &mut RecipeBook, data: CraftingData) {
    if data.clear_recipes {
        book.clear();
    }
    let shaped = data.shaped_recipes.into_iter().chain(data.shaped_chemistry_recipes);
    for recipe in shaped {
        book.push(shaped_recipe(recipe));
    }
    let shapeless = data.shapeless_recipes.into_iter().chain(data.shulker_box_recipes).chain(data.shapeless_chemistry_recipes);
    for recipe in shapeless {
        book.push(shapeless_recipe(recipe));
    }
    for r in data.smithing_transform_recipes {
        book.push(Recipe {
            network_id: r.network_id,
            id: r.recipe_id,
            kind: RecipeKind::SmithingTransform,
            block: r.tag,
            priority: 0,
            inputs: [r.template, r.base, r.addition].iter().map(ingredient).collect(),
            outputs: vec![ItemStack::from(&r.result)],
        });
    }
    for m in data.multi_recipes {
        book.multi.insert(m.uuid.to_string(), m.network_id);
    }
    for r in data.smithing_trim_recipes {
        book.push(Recipe {
            network_id: r.network_id,
            id: r.recipe_id,
            kind: RecipeKind::SmithingTrim,
            block: r.block,
            priority: 0,
            inputs: [r.template, r.input, r.addition].iter().map(ingredient).collect(),
            outputs: Vec::new(),
        });
    }
}

fn shaped_recipe(r: ShapedRecipe) -> Recipe {
    let dim = |v: i32| u8::try_from(v).unwrap_or(0);
    Recipe {
        network_id: r.network_id,
        kind: RecipeKind::Shaped { width: dim(r.width), height: dim(r.height) },
        block: r.block,
        priority: r.priority,
        inputs: r.input.iter().map(ingredient).collect(),
        outputs: outputs(&r.output),
        id: r.recipe_id,
    }
}

fn shapeless_recipe(r: ShapelessRecipe) -> Recipe {
    Recipe {
        network_id: r.network_id,
        kind: RecipeKind::Shapeless,
        block: r.block,
        priority: r.priority,
        inputs: r.input.iter().map(ingredient).collect(),
        outputs: outputs(&r.output),
        id: r.recipe_id,
    }
}

fn outputs(items: &[ItemLegacy]) -> Vec<ItemStack> {
    items.iter().map(ItemStack::from).collect()
}

fn ingredient(i: &RecipeIngredient) -> Ingredient {
    let count = u16::try_from(i.count).unwrap_or(0);
    let metadata = |m: i32| (m != ANY_METADATA && m != -1).then_some(m);
    let C::Valid(valid) = &i.content else { return Ingredient::Empty };
    match &valid.content {
        V::Name(n) => Ingredient::Item { name: n.name.clone(), metadata: metadata(n.metadata), count },
        V::ItemTag(t) => Ingredient::Tag { tag: t.tag.clone(), count },
        V::Molang(m) => Ingredient::Molang { expression: m.expression.clone(), version: m.version, count },
        V::Empty(_) | V::Default => Ingredient::Empty,
    }
}
