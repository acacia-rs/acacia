//! The craft action that heads a crafting request, and the `CraftResultsDeprecated` list vanilla
//! sends right after it. Sequences per workstation: docs/research/workstations.md.

use acacia_client::proto::types::{
    ItemExtraDataWithoutBlockingTick, ItemExtraDataWithoutBlockingTickHasNbt as HasNbt, ItemExtraDataWithoutBlockingTickNbt,
    ItemStackRequestActionsItem as Action, ItemStackRequestActionsItemContent as Content,
    ItemStackRequestActionsItemContentCraftGrindstoneRequest, ItemStackRequestActionsItemContentCraftLoomRequest,
    ItemStackRequestActionsItemContentCraftRecipe, ItemStackRequestActionsItemContentCraftRecipeAuto,
    ItemStackRequestActionsItemContentOptional, ItemStackRequestActionsItemContentResultsDeprecated,
    ItemStackRequestActionsItemTypeId as TypeId, ItemStackRequestCause, ItemStackRequestInstanceDescriptor,
    ItemStackRequestInstanceDescriptorContent, ItemStackRequestInstanceDescriptorType, RecipeIngredient2,
    RecipeIngredient2Content, RecipeIngredient2ContentItemTag, RecipeIngredient2ContentMolang, RecipeIngredient2ContentName,
    RecipeIngredient2Type,
};

use super::request::{action, Texts};
use crate::state::{Ingredient, ItemStack};

/// Aux value meaning "any metadata" in item descriptors.
const ANY_METADATA: i32 = 32767;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CraftAction {
    /// `CraftRecipe`: stonecutter, smithing table, enchanting option, trade.
    Recipe { network_id: u32, times: u8 },
    /// `CraftRecipeAuto`: a recipe-book craft; `ingredients` are the recipe's non-empty cells.
    Auto { network_id: u32, times: u8, ingredients: Vec<Ingredient> },
    /// `CraftRecipeOptional`: anvil, cartography; `filter_index` points into the request's custom names.
    Optional { network_id: u32, filter_index: i32 },
    Grindstone { network_id: i32, times: u8, cost: i32 },
    Loom { pattern: String, times: u8 },
}

impl CraftAction {
    pub fn times(&self) -> u8 {
        match self {
            CraftAction::Recipe { times, .. }
            | CraftAction::Auto { times, .. }
            | CraftAction::Grindstone { times, .. }
            | CraftAction::Loom { times, .. } => *times,
            CraftAction::Optional { .. } => 1,
        }
    }
}

/// A craft: its action, results and request texts.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Craft {
    pub action: CraftAction,
    /// `(identifier, stack)` per result, counted for one craft.
    pub results: Vec<(String, ItemStack)>,
    pub texts: Texts,
    /// Send `CraftResultsDeprecated` after the craft action.
    pub results_action: bool,
}

impl Craft {
    pub fn new(action: CraftAction, results: Vec<(String, ItemStack)>) -> Self {
        Craft { action, results, texts: Texts::default(), results_action: false }
    }

    /// Adds `CraftResultsDeprecated`, only for vanilla-likeness where vanilla sends it (grid,
    /// recipe book, enchanting, grindstone, smithing, loom, trade): BDS ignores it there, only its
    /// chemistry stations read it, and on the anvil, stonecutter and cartography table it crashes
    /// BDS 1.26.52.
    pub fn with_results_action(mut self) -> Self {
        self.results_action = true;
        self
    }

    /// Anvil rename: the name travels as the request's only filter string.
    pub fn renaming(mut self, name: &str) -> Self {
        self.texts = Texts { custom_names: vec![name.to_owned()], cause: ItemStackRequestCause::AnvilText };
        self
    }

    /// Cartography: every craft carries the result's name as its only filter string, `""` when
    /// it keeps its name.
    pub fn cartography_name(mut self, name: &str) -> Self {
        self.texts = Texts { custom_names: vec![name.to_owned()], cause: ItemStackRequestCause::CartographyText };
        self
    }

    /// What lands in the created-output slot for each result: all crafts at once.
    pub fn created(&self) -> Vec<ItemStack> {
        let times = u16::from(self.action.times());
        self.results.iter().map(|(_, stack)| ItemStack { count: stack.count * times, ..stack.clone() }).collect()
    }

    /// The craft action, then `CraftResultsDeprecated` unless left out.
    pub fn actions(&self) -> Vec<Action> {
        if !self.results_action {
            return vec![self.craft_action()];
        }
        let result_items = self.results.iter().map(|(name, stack)| descriptor(name, stack)).collect();
        // Per-craft results; vanilla repeats the craft count only for recipe-book crafts (2026-10-02 capture).
        let times_crafted = if let CraftAction::Auto { times, .. } = self.action { times } else { 1 };
        let results = Content::ResultsDeprecated(ItemStackRequestActionsItemContentResultsDeprecated { result_items, times_crafted });
        vec![self.craft_action(), action(TypeId::ResultsDeprecated, results)]
    }

    fn craft_action(&self) -> Action {
        match &self.action {
            &CraftAction::Recipe { network_id, times } => action(
                TypeId::CraftRecipe,
                Content::CraftRecipe(ItemStackRequestActionsItemContentCraftRecipe { recipe_network_id: network_id, times_crafted: times }),
            ),
            CraftAction::Auto { network_id, times, ingredients } => action(
                TypeId::CraftRecipeAuto,
                Content::CraftRecipeAuto(ItemStackRequestActionsItemContentCraftRecipeAuto {
                    recipe_network_id: *network_id,
                    times_crafted: *times,
                    ingredients: ingredients.iter().map(ingredient).collect(),
                }),
            ),
            &CraftAction::Optional { network_id, filter_index } => action(
                TypeId::Optional,
                Content::Optional(ItemStackRequestActionsItemContentOptional { recipe_network_id: network_id, filtered_string_index: filter_index }),
            ),
            &CraftAction::Grindstone { network_id, times, cost } => action(
                TypeId::CraftGrindstoneRequest,
                Content::CraftGrindstoneRequest(ItemStackRequestActionsItemContentCraftGrindstoneRequest {
                    recipe_network_id: network_id,
                    times_crafted: times,
                    cost,
                }),
            ),
            CraftAction::Loom { pattern, times } => action(
                TypeId::CraftLoomRequest,
                Content::CraftLoomRequest(ItemStackRequestActionsItemContentCraftLoomRequest { pattern: pattern.clone(), times_crafted: *times }),
            ),
        }
    }
}

fn descriptor(name: &str, stack: &ItemStack) -> ItemStackRequestInstanceDescriptor {
    let kind = ItemStackRequestInstanceDescriptorType::Name;
    let nbt = stack.nbt.clone().map(|nbt| ItemExtraDataWithoutBlockingTickNbt { version: 1, nbt });
    let extra = ItemExtraDataWithoutBlockingTick {
        has_nbt: if nbt.is_some() { HasNbt::True } else { HasNbt::False },
        nbt,
        can_place_on: Vec::new(),
        can_destroy: Vec::new(),
    };
    ItemStackRequestInstanceDescriptor {
        r#type: kind,
        legacy_type: kind.to_raw() as u8,
        content: Some(ItemStackRequestInstanceDescriptorContent { name: name.to_owned(), metadata: stack.metadata as i32 }),
        count: stack.count as i16,
        block_runtime_id: stack.block_runtime_id,
        extra: Some(extra),
    }
}

fn ingredient(i: &Ingredient) -> RecipeIngredient2 {
    let (kind, content) = match i {
        Ingredient::Empty => (RecipeIngredient2Type::Invalid, RecipeIngredient2Content::Invalid),
        Ingredient::Item { name, metadata, .. } => (
            RecipeIngredient2Type::Name,
            RecipeIngredient2Content::Name(RecipeIngredient2ContentName { name: name.clone(), metadata: metadata.unwrap_or(ANY_METADATA) }),
        ),
        Ingredient::Tag { tag, .. } => {
            (RecipeIngredient2Type::ItemTag, RecipeIngredient2Content::ItemTag(RecipeIngredient2ContentItemTag { tag: tag.clone() }))
        }
        Ingredient::Molang { expression, version, .. } => (
            RecipeIngredient2Type::Molang,
            RecipeIngredient2Content::Molang(RecipeIngredient2ContentMolang { expression: expression.clone(), version: *version }),
        ),
    };
    RecipeIngredient2 { r#type: kind, legacy_type: kind.to_raw() as u8, content, count: i.count() }
}
