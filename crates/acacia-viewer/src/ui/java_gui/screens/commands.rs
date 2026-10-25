//! What the Java screens' `slotClicked`s ask of the menu, as the bot's commands. The bot clicks
//! left, right and shift; the rest is made of those where it can be:
//! - a quick-craft drag: right clicks, the split's share on each slot (a full stack each: no);
//! - a hotbar or offhand key's swap: pick up, put down on the other slot, put back;
//! - the drop key over a slot: pick up, throw one or all, put back;
//! - double-click's pick-all and the pick key's clone: no.

use acacia_bot::items::{Click as BotClick, SlotRef, ui};
use acacia_bot::workstation::BeaconEffect;
use acacia_ui::java::container::input::{Click, Input};

use super::{Target, View};
use crate::control::Command;

/// The bot's slot of a hotbar key's button (0-8) or the offhand's (40).
fn swap_slot(button: i32) -> Option<SlotRef> {
    match button {
        0..=8 => Some(SlotRef::Main(button as u8)),
        40 => Some(SlotRef::Offhand),
        _ => None,
    }
}

pub(super) fn commands(click: &Click, targets: &[Target], view: &View) -> Vec<Command> {
    let carrying = view.inventory.cursor.is_some();
    match click {
        // Slot -999: the carried stack thrown, one of it for the right button.
        Click::Slot { slot: None, outside: true, button, .. } if carrying => vec![Command::DropCursor { one: *button == 1 }],
        Click::Slot { slot: Some(i), button, input, .. } => slot(targets.get(*i).unwrap_or(&Target::None), *button, *input, view),
        Click::QuickCraft { kind, slots } => quick_craft(*kind, slots, targets, view),
        Click::Slot { .. } => Vec::new(),
    }
}

fn slot(target: &Target, button: i32, input: Input, view: &View) -> Vec<Command> {
    let carrying = view.inventory.cursor.is_some();
    let all = input == Input::QuickMove;
    match (target, input) {
        (Target::Slot(r), Input::Pickup | Input::QuickMove) => {
            let click = match (input, button) {
                (Input::QuickMove, _) => BotClick::Shift,
                (_, 1) => BotClick::Right,
                _ => BotClick::Left,
            };
            // A beacon takes one item: the held stack is not put down whole.
            let click = if *r == SlotRef::Ui(ui::BEACON_PAYMENT) && carrying && !all { BotClick::Right } else { click };
            vec![Command::Click(*r, click)]
        }
        (Target::Slot(r), Input::Swap) => match swap_slot(button) {
            Some(other) if other != *r => [*r, other, *r].map(|s| Command::Click(s, BotClick::Left)).into(),
            _ => Vec::new(),
        },
        (Target::Slot(r), Input::Throw) if !carrying => {
            vec![Command::Click(*r, BotClick::Left), Command::DropCursor { one: button == 0 }, Command::Click(*r, BotClick::Left)]
        }
        (Target::Crafted, Input::Pickup | Input::QuickMove) => {
            let name = Some(view.name.0.clone()).filter(|n| !n.is_empty() && view.inventory.bench == Some(acacia_ui::inventory::Bench::Anvil));
            vec![Command::TakeCrafted { all, name }]
        }
        (Target::Cut(Some(id)), Input::Pickup | Input::QuickMove) => vec![Command::TakeCut { id: *id, all }],
        (Target::Loom(Some(code)), Input::Pickup | Input::QuickMove) => vec![Command::Loom((*code).to_owned())],
        (Target::Creative(_) | Target::Destroy, Input::Pickup | Input::QuickMove) if carrying => vec![Command::DestroyCursor],
        (Target::Creative(id), Input::Pickup | Input::QuickMove) => vec![Command::Creative { id: *id, count: if all { 64 } else { 1 } }],
        _ => Vec::new(),
    }
}

/// `quickCraftToSlots` as right clicks: kind 0 splits the stack evenly, kind 1 lays one on each.
fn quick_craft(kind: i32, slots: &[usize], targets: &[Target], view: &View) -> Vec<Command> {
    let count = view.inventory.cursor.as_ref().map_or(0, |c| usize::from(c.count));
    let each = match kind {
        0 => count / slots.len().max(1),
        1 => 1,
        _ => 0,
    };
    let refs = slots.iter().filter_map(|&i| match targets.get(i) {
        Some(Target::Slot(r)) => Some(*r),
        _ => None,
    });
    refs.flat_map(|r| std::iter::repeat_n(Command::Click(r, BotClick::Right), each)).collect()
}

/// The bot's beacon power of a Java effect path.
pub(super) fn beacon_effect(path: &str) -> Option<BeaconEffect> {
    Some(match path {
        "speed" => BeaconEffect::Speed,
        "haste" => BeaconEffect::Haste,
        "resistance" => BeaconEffect::Resistance,
        "jump_boost" => BeaconEffect::JumpBoost,
        "strength" => BeaconEffect::Strength,
        "regeneration" => BeaconEffect::Regeneration,
        _ => return None,
    })
}
