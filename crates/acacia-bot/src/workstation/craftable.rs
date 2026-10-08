//! What the recipe book offers now: crafting recipes whose ingredients the inventory holds.

use crate::state::{ItemStack, Recipe, RecipeKind};
use crate::Bot;

impl Bot {
    /// What the player can craft now, one result stack per item, in the server's recipe order: in
    /// the own 2x2 grid, or with `table` at a crafting table. Ingredients are matched greedily,
    /// so a recipe whose tags overlap may be missed when the stacks would only fit another way round.
    pub fn craftable(&self, table: bool) -> Vec<ItemStack> {
        let state = self.state();
        let mut out: Vec<ItemStack> = Vec::new();
        for recipe in state.recipes.book().iter().filter(|r| r.block == "crafting_table" && (table || r.fits_2x2())) {
            let Some(result) = recipe.outputs.first() else { continue };
            if !out.iter().any(|o| o.network_id == result.network_id) && self.affords(recipe) {
                out.push(result.clone());
            }
        }
        out
    }

    fn affords(&self, recipe: &Recipe) -> bool {
        if !matches!(recipe.kind, RecipeKind::Shaped { .. } | RecipeKind::Shapeless) {
            return false;
        }
        let state = self.state();
        let mut left: Vec<(&str, u32, u16)> = state
            .inventory
            .main
            .iter()
            .filter(|s| !s.is_empty())
            .filter_map(|s| Some((state.item_name(s)?, s.metadata, s.count)))
            .collect();
        recipe.inputs.iter().filter(|i| !i.is_empty()).all(|ingredient| {
            let mut need = ingredient.count();
            for (_, _, count) in left.iter_mut().filter(|(n, a, c)| *c > 0 && ingredient.accepts(n, *a)) {
                let take = need.min(*count);
                (*count, need) = (*count - take, need - take);
                if need == 0 {
                    break;
                }
            }
            need == 0
        })
    }
}
