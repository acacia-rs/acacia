//! Enchantment tables the anvil and grindstone rules need:
//! max levels, rarity weights for the anvil cost, exclusive sets and curses. Ids are Bedrock's.

use acacia_client::proto::nbt::{List, Nbt, Value};

use crate::state::ItemStack;

pub(crate) const BINDING: i16 = 27;
pub(crate) const VANISHING: i16 = 28;
const COMPOUND_TAG: u8 = 10;

#[derive(Clone, Copy)]
enum Rarity {
    Common,
    Uncommon,
    Rare,
    VeryRare,
}

use Rarity::*;

/// `(Java registry name, max level, rarity)` by Bedrock enchantment id 0-40.
const TABLE: [(&str, i16, Rarity); 41] = [
    ("protection", 4, Common),
    ("fire_protection", 4, Uncommon),
    ("feather_falling", 4, Uncommon),
    ("blast_protection", 4, Rare),
    ("projectile_protection", 4, Uncommon),
    ("thorns", 3, VeryRare),
    ("respiration", 3, Rare),
    ("depth_strider", 3, Rare),
    ("aqua_affinity", 1, Rare),
    ("sharpness", 5, Common),
    ("smite", 5, Uncommon),
    ("bane_of_arthropods", 5, Uncommon),
    ("knockback", 2, Uncommon),
    ("fire_aspect", 2, Rare),
    ("looting", 3, Rare),
    ("efficiency", 5, Common),
    ("silk_touch", 1, VeryRare),
    ("unbreaking", 3, Uncommon),
    ("fortune", 3, Rare),
    ("power", 5, Common),
    ("punch", 2, Rare),
    ("flame", 1, Rare),
    ("infinity", 1, VeryRare),
    ("luck_of_the_sea", 3, Rare),
    ("lure", 3, Rare),
    ("frost_walker", 2, Rare),
    ("mending", 1, Rare),
    ("binding_curse", 1, VeryRare),
    ("vanishing_curse", 1, VeryRare),
    ("impaling", 5, Rare),
    ("riptide", 3, Rare),
    ("loyalty", 3, Uncommon),
    ("channeling", 1, VeryRare),
    ("multishot", 1, Rare),
    ("piercing", 4, Common),
    ("quick_charge", 3, Uncommon),
    ("soul_speed", 3, VeryRare),
    ("swift_sneak", 3, VeryRare),
    ("wind_burst", 3, Rare),
    ("density", 5, Uncommon),
    ("breach", 4, Rare),
];

/// Sets of enchantments no item may carry two of.
const EXCLUSIVE: [&[i16]; 8] = [&[0, 1, 3, 4], &[9, 10, 11, 39, 40], &[16, 18], &[22, 26], &[7, 25], &[30, 31], &[30, 32], &[33, 34]];

/// Bedrock's id for a Java enchantment registry key; `None` for Java-only ones (sweeping edge, lunge).
#[cfg(feature = "java")]
pub(crate) fn bedrock_id(java_name: &str) -> Option<i16> {
    let name = java_name.strip_prefix("minecraft:").unwrap_or(java_name);
    TABLE.iter().position(|e| e.0 == name).and_then(|i| i16::try_from(i).ok())
}

pub(crate) fn is_curse(id: i16) -> bool {
    id == BINDING || id == VANISHING
}

fn compatible(a: i16, b: i16) -> bool {
    a == b || !EXCLUSIVE.iter().any(|set| set.contains(&a) && set.contains(&b))
}

impl ItemStack {
    /// `(id, level)` of every enchantment it carries.
    pub fn enchantments(&self) -> Vec<(i16, i16)> {
        enchants(self)
    }
}

/// `(id, level)` of every enchantment in the stack's `ench` list.
pub(crate) fn enchants(stack: &ItemStack) -> Vec<(i16, i16)> {
    let Some(Value::List(list)) = stack.nbt.as_ref().and_then(|n| n.value.get("ench")) else { return Vec::new() };
    let short = |e: &Value, key| match e.get(key) {
        Some(Value::Short(v)) => Some(*v),
        _ => None,
    };
    list.items.iter().filter_map(|e| Some((short(e, "id")?, short(e, "lvl")?))).collect()
}

/// `stack` with its `ench` list replaced by `enchants`; an empty list removes the key.
pub(crate) fn with_enchant_list(stack: &ItemStack, enchants: &[(i16, i16)]) -> ItemStack {
    let mut result = stack.clone();
    let root = result.nbt.get_or_insert_with(|| Nbt { name: String::new(), value: Value::Compound(Vec::new()) });
    let Value::Compound(entries) = &mut root.value else { return result };
    entries.retain(|(k, _)| k != "ench");
    if !enchants.is_empty() {
        let items = enchants.iter().map(|&(id, lvl)| Value::Compound(vec![("id".into(), Value::Short(id)), ("lvl".into(), Value::Short(lvl))])).collect();
        entries.push(("ench".into(), Value::List(List { tag: COMPOUND_TAG, items })));
        entries.sort_by(|a, b| a.0.cmp(&b.0));
    }
    result
}

/// The anvil's merge of `added`'s enchantments onto `base`'s: equal levels go up one (to the
/// max), else the higher stays. Errs on an enchantment exclusive with one already there.
pub(crate) fn merge(base: &[(i16, i16)], added: &[(i16, i16)]) -> Result<Vec<(i16, i16)>, String> {
    let mut out = base.to_vec();
    for &(id, lvl) in added {
        if let Some(e) = out.iter_mut().find(|e| e.0 == id) {
            e.1 = if e.1 == lvl { (lvl + 1).min(max_level(id)) } else { e.1.max(lvl) };
        } else if out.iter().all(|&(other, _)| compatible(id, other)) {
            out.push((id, lvl));
        } else {
            return Err(format!("enchantment {id} conflicts with the item's"));
        }
    }
    Ok(out)
}

/// The anvil's enchantment value: Σ weight × level, weights by rarity (book weights when the
/// material is an enchanted book).
pub(crate) fn anvil_value(enchants: &[(i16, i16)], book: bool) -> i32 {
    enchants.iter().map(|&(id, lvl)| weight(id, book) * i32::from(lvl)).sum()
}

fn max_level(id: i16) -> i16 {
    usize::try_from(id).ok().and_then(|i| TABLE.get(i)).map_or(1, |e| e.1)
}

fn weight(id: i16, book: bool) -> i32 {
    let rarity = usize::try_from(id).ok().and_then(|i| TABLE.get(i)).map_or(Common, |e| e.2);
    match (rarity, book) {
        (VeryRare, false) => 8,
        (VeryRare, true) | (Rare, false) => 4,
        (Rare, true) | (Uncommon, false) => 2,
        _ => 1,
    }
}
