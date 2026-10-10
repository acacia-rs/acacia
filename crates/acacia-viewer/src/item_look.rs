//! What sets a stack's look apart from others of its name and aux: leather's dye and a banner's
//! patterns, both from the item NBT.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use acacia_bot::Bot;
use acacia_bot::state::ItemStack;
use acacia_render::banner::Banner;
use acacia_render::item::ItemKey;

const BANNER: &str = "minecraft:banner";

/// A banner item's cloth: its aux is the base dye, its NBT has the patterns. `None` for other items.
pub fn banner(name: &str, stack: &ItemStack) -> Option<Arc<Banner>> {
    (name == BANNER).then(|| Arc::new(crate::block_data::item_banner(stack.metadata, stack.nbt.as_ref())))
}

/// The item model's key; `None` for an empty stack.
pub fn key(bot: &Bot, stack: &ItemStack) -> Option<ItemKey> {
    let name = bot.state().item_name(stack).filter(|_| !stack.is_empty())?.to_owned();
    let banner = banner(&name, stack);
    Some(ItemKey { name, aux: stack.metadata, block: crate::control::block_of(bot, stack), dye: stack.custom_color(), banner })
}

/// Names the look in a cache key; empty for a stack that looks like any other of its kind.
pub fn variant(dye: Option<[u8; 3]>, banner: Option<&Banner>) -> String {
    match (dye, banner) {
        (Some([r, g, b]), _) => format!("{r:02x}{g:02x}{b:02x}"),
        (None, Some(banner)) if !banner.layers.is_empty() || banner.ominous => {
            let mut hasher = DefaultHasher::new();
            banner.hash(&mut hasher);
            format!("{:x}", hasher.finish())
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_dyed_and_patterned_stacks_get_a_look_of_their_own() {
        let plain = Banner::from_bedrock(1, [], 0);
        let striped = Banner::from_bedrock(1, [("bs", 0)], 0);
        assert_eq!((variant(None, None), variant(None, Some(&plain))), (String::new(), String::new()));
        assert_eq!(variant(Some([255, 0, 16]), None), "ff0010");
        assert!(!variant(None, Some(&striped)).is_empty() && variant(None, Some(&striped)) != variant(None, Some(&Banner::from_bedrock(1, [("ts", 0)], 0))));
    }
}
