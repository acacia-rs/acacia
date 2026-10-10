//! What sets a stack's look apart from others of its name and aux: leather's dye and a banner's
//! patterns, both from the item NBT.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use acacia_bot::Bot;
use acacia_bot::state::ItemStack;
use acacia_render::banner::Banner;
use acacia_render::item::{ItemKey, SHIELD};

const BANNER: &str = "minecraft:banner";

/// A banner item's cloth (its aux is the base dye, its NBT has the patterns), or the banner a
/// shield was crafted with. `None` for other items.
pub fn banner(name: &str, stack: &ItemStack) -> Option<Arc<Banner>> {
    match name {
        BANNER => Some(crate::block_data::item_banner(stack.metadata, stack.nbt.as_ref())),
        SHIELD => crate::block_data::shield_banner(stack.nbt.as_ref()?),
        _ => None,
    }
    .map(Arc::new)
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
        (None, Some(banner)) => {
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
    fn dyed_and_bannered_stacks_get_a_look_of_their_own() {
        let plain = Banner::from_bedrock(1, [], 0);
        let striped = Banner::from_bedrock(1, [("bs", 0)], 0);
        // A shield's plain banner still differs from no banner.
        assert!(variant(None, None).is_empty() && !variant(None, Some(&plain)).is_empty());
        assert_eq!(variant(Some([255, 0, 16]), None), "ff0010");
        assert!(!variant(None, Some(&striped)).is_empty() && variant(None, Some(&striped)) != variant(None, Some(&Banner::from_bedrock(1, [("ts", 0)], 0))));
    }
}
