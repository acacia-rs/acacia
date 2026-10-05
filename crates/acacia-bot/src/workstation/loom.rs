//! Loom: `CraftLoom` with the Bedrock pattern id (`bo`, `cr`, ...). Geyser reads the new pattern
//! from the banner in `CraftResultsDeprecated`, so the result carries the updated `Patterns` NBT.

use acacia_client::proto::nbt::{List, Nbt, Value};
use acacia_client::proto::types::WindowType;
use acacia_physics::BlockPos;

use super::{named, occupied};
use crate::items::craft::{Craft, CraftAction};
use crate::items::{to_inventory_ops, ui, Op, SlotRef};
use crate::state::{GameState, ItemStack};
use crate::{ActionError, Bot};

const COMPOUND_TAG: u8 = 10;

impl Bot {
    /// Adds `pattern` (Bedrock pattern id such as `bo`) in the colour of `dye` to the banner in
    /// `banner` at the loom at `pos`; `pattern_item` is the banner pattern some designs need.
    pub async fn loom(&mut self, pos: BlockPos, banner: SlotRef, dye: SlotRef, pattern: &str, pattern_item: Option<SlotRef>) -> Result<(), ActionError> {
        let slots = [ui::LOOM_BANNER, ui::LOOM_DYE, ui::LOOM_PATTERN].map(SlotRef::Ui);
        self.workstation_craft(pos, WindowType::Loom, &[Some(banner), Some(dye), pattern_item], &slots, |state| loom_plan(state, pattern)).await
    }
}

pub(crate) fn loom_plan(state: &GameState, pattern: &str) -> Result<(Craft, Vec<Op>), ActionError> {
    let (banner_slot, dye_slot) = (SlotRef::Ui(ui::LOOM_BANNER), SlotRef::Ui(ui::LOOM_DYE));
    let (banner, dye) = (occupied(state, banner_slot)?, occupied(state, dye_slot)?);
    let dye_name = state.items.name(dye.network_id).unwrap_or_default();
    let color = dye_color(dye_name).ok_or_else(|| ActionError::NotPossible(format!("{dye_name} is not a dye")))?;
    let result = ItemStack { count: 1, nbt: Some(with_pattern(banner.nbt.as_ref(), pattern, color)), ..banner.clone() };
    let craft = Craft::new(CraftAction::Loom { pattern: pattern.to_owned(), times: 1 }, vec![named(state, &result)?]).with_results_action();
    let mut ops = vec![Op::Consume { from: banner_slot, count: 1 }, Op::Consume { from: dye_slot, count: 1 }];
    ops.extend(to_inventory_ops(state, SlotRef::CREATED_OUTPUT, &result)?);
    Ok((craft, ops))
}

/// Banner pattern colour of a dye item (Bedrock numbering: 0 black ... 15 white).
pub(crate) fn dye_color(name: &str) -> Option<i32> {
    const COLORS: [&str; 16] = [
        "black", "red", "green", "brown", "blue", "purple", "cyan", "light_gray", "gray", "pink", "lime", "yellow", "light_blue",
        "magenta", "orange", "white",
    ];
    let item = name.strip_prefix("minecraft:")?;
    let legacy = match item {
        "ink_sac" => Some("black"),
        "cocoa_beans" => Some("brown"),
        "lapis_lazuli" => Some("blue"),
        "bone_meal" => Some("white"),
        _ => None,
    };
    let color = legacy.or_else(|| item.strip_suffix("_dye"))?;
    COLORS.iter().position(|c| *c == color).map(|i| i as i32)
}

/// `nbt` (the banner's, if any) with `{Pattern, Color}` appended to its `Patterns` list.
fn with_pattern(nbt: Option<&Nbt>, pattern: &str, color: i32) -> Nbt {
    let mut root = nbt.cloned().unwrap_or_else(|| Nbt { name: String::new(), value: Value::Compound(Vec::new()) });
    let Value::Compound(entries) = &mut root.value else { return root };
    let layer = Value::Compound(vec![("Color".into(), Value::Int(color)), ("Pattern".into(), Value::String(pattern.into()))]);
    match entries.iter_mut().find(|(k, _)| k == "Patterns") {
        Some((_, Value::List(list))) => list.items.push(layer),
        _ => {
            // Vanilla's result lists `Patterns` before `Type` (keys in byte order).
            entries.push(("Patterns".into(), Value::List(List { tag: COMPOUND_TAG, items: vec![layer] })));
            entries.sort_by(|a, b| a.0.cmp(&b.0));
        }
    }
    root
}
