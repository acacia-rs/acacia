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

/// `(max level, rarity)` by enchantment id 0-40.
const TABLE: [(i16, Rarity); 41] = [
    (4, Common),   // protection
    (4, Uncommon), // fire_protection
    (4, Uncommon), // feather_falling
    (4, Rare),     // blast_protection
    (4, Uncommon), // projectile_protection
    (3, VeryRare), // thorns
    (3, Rare),     // respiration
    (3, Rare),     // depth_strider
    (1, Rare),     // aqua_affinity
    (5, Common),   // sharpness
    (5, Uncommon), // smite
    (5, Uncommon), // bane_of_arthropods
    (2, Uncommon), // knockback
    (2, Rare),     // fire_aspect
    (3, Rare),     // looting
    (5, Common),   // efficiency
    (1, VeryRare), // silk_touch
    (3, Uncommon), // unbreaking
    (3, Rare),     // fortune
    (5, Common),   // power
    (2, Rare),     // punch
    (1, Rare),     // flame
    (1, VeryRare), // infinity
    (3, Rare),     // luck_of_the_sea
    (3, Rare),     // lure
    (2, Rare),     // frost_walker
    (1, Rare),     // mending
    (1, VeryRare), // binding
    (1, VeryRare), // vanishing
    (5, Rare),     // impaling
    (3, Rare),     // riptide
    (3, Uncommon), // loyalty
    (1, VeryRare), // channeling
    (1, Rare),     // multishot
    (4, Common),   // piercing
    (3, Uncommon), // quick_charge
    (3, VeryRare), // soul_speed
    (3, VeryRare), // swift_sneak
    (3, Rare),     // wind_burst
    (5, Uncommon), // density
    (4, Rare),     // breach
];

/// Sets of enchantments no item may carry two of.
const EXCLUSIVE: [&[i16]; 8] = [&[0, 1, 3, 4], &[9, 10, 11, 39, 40], &[16, 18], &[22, 26], &[7, 25], &[30, 31], &[30, 32], &[33, 34]];

pub(crate) fn is_curse(id: i16) -> bool {
    id == BINDING || id == VANISHING
}

fn compatible(a: i16, b: i16) -> bool {
    a == b || !EXCLUSIVE.iter().any(|set| set.contains(&a) && set.contains(&b))
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
    usize::try_from(id).ok().and_then(|i| TABLE.get(i)).map_or(1, |e| e.0)
}

fn weight(id: i16, book: bool) -> i32 {
    let rarity = usize::try_from(id).ok().and_then(|i| TABLE.get(i)).map_or(Common, |e| e.1);
    match (rarity, book) {
        (VeryRare, false) => 8,
        (VeryRare, true) | (Rare, false) => 4,
        (Rare, true) | (Uncommon, false) => 2,
        _ => 1,
    }
}
