//! The bot thread's side of playing: commands from the window and the screens' contents (the
//! player as each tick leaves it is me.rs).

use acacia_bot::Bot;
use acacia_bot::forms::FormReply;
use acacia_bot::interact::Face;
use acacia_bot::items::{Click, SlotRef};
use acacia_bot::state::ItemStack;
use acacia_bot::movement::Controls;
use acacia_bot::proto::types::WindowType;
use acacia_ui::inventory::{Bench, Progress, Station};
use glam::IVec3;

/// What the player at the window does.
#[derive(Debug, Clone)]
pub enum Command {
    /// Movement keys and aim; kept until the next one.
    Controls(Controls),
    Hotbar(u8),
    /// Attack held on this block face, or released.
    Mine(Option<(IVec3, Face)>),
    /// Right-click on a block face.
    UseOn(IVec3, Face),
    /// Left- (`attack`) or right-click on an entity.
    Entity { runtime_id: u64, attack: bool },
    /// Right-click on nothing, and its release.
    UseItem,
    ReleaseItem,
    /// Left-click on nothing.
    Swing,
    /// Sneak pressed on a vehicle.
    Dismount,
    /// The death screen's respawn button.
    Respawn,
    Chat(String),
    /// The inventory screen opened (E) or closed.
    Inventory(bool),
    /// A click on a slot of the open screen.
    Click(SlotRef, Click),
    /// A recipe-book click: one craft of item `name`, at the crafting table at `table` if given.
    Craft { name: String, table: Option<[i32; 3]> },
    /// A click on what the workstation's slots make: one craft, or all a crafting grid holds.
    TakeCrafted { all: bool, name: Option<String> },
    /// The text in an anvil's name box changed (`None`: empty); handled by the bot loop, for the
    /// result shown.
    Name(Option<String>),
    /// A click on the stonecutter's result with recipe `id` picked: one cut, or all.
    TakeCut { id: u32, all: bool },
    /// The right-click that opens the held book, and a page of a book and quill as typed.
    OpenBook,
    EditBook(acacia_bot::books::PageEdit),
    /// A click on the loom's result with this pattern picked.
    Loom(String),
    /// A click on the enchanting table's option of this index.
    Enchant(usize),
    /// A click on a trader's offer of this index: one trade.
    Trade(usize),
    /// The beacon screen's confirm with these powers.
    Beacon(acacia_bot::workstation::BeaconEffect, Option<acacia_bot::workstation::BeaconEffect>),
    /// A click on a creative inventory entry: this many of it into the inventory.
    Creative { id: u32, count: u8 },
    /// A click outside the screen: throws the held stack, or one of it.
    DropCursor { one: bool },
    /// The player's answer to open form `id`.
    AnswerForm(u32, FormReply),
    /// The open sign editor closed with this text, or (`None`) with the side's text unchanged.
    WriteSign(Option<String>),
}

/// The player's own slots as a screen shows them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Inventory {
    /// 0-8 the hotbar.
    pub main: Vec<Option<Stack>>,
    pub armor: [Option<Stack>; 4],
    pub offhand: Option<Stack>,
    pub cursor: Option<Stack>,
    /// The open container, when it is rows of nine (chests, barrels, shulker boxes) or a station.
    pub container: Option<Rows>,
    /// The open container is this workstation, and how far its work has come.
    pub station: Option<Station>,
    pub progress: Progress,
    /// An open crafting table's position.
    pub workbench: Option<[i32; 3]>,
    /// The workstation slots of the player's UI window, by offset.
    pub ui: Vec<Option<Stack>>,
    /// What can be crafted now (in the 2x2 grid, or at the open table); filled by [`craftable`].
    pub craftable: Vec<Stack>,
    /// What the crafting grid's or the bench's slots make; filled by [`crafted`].
    pub crafted: Option<Stack>,
    /// The open workstation whose slots are the player's UI slots.
    pub bench: Option<Bench>,
    /// What a stonecutter offers for its input: recipe ids and one cut's result.
    pub picks: Vec<(u32, Stack)>,
    /// The patterns an open loom offers.
    pub patterns: Vec<&'static str>,
    /// The level each option of an enchanting table needs.
    pub enchants: Vec<u8>,
    /// An open trading screen.
    pub trade: Option<crate::stations::Trade>,
}

pub fn crafted(bot: &Bot, name: Option<&str>) -> Option<Stack> {
    bot.station_result(name).and_then(|s| stack_of(bot, &s))
}

/// One result stack per item [`Inventory`]'s crafting can make now.
pub fn craftable(bot: &Bot, table: bool) -> Vec<Stack> {
    bot.craftable(table).iter().filter_map(|s| stack_of(bot, s)).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rows {
    pub title: String,
    pub slots: Vec<Option<Stack>>,
}

/// A stack's block id in the world's registry: items carry the server's wire id, hashed on BDS.
pub fn block_of(bot: &Bot, s: &ItemStack) -> u32 {
    let Some(view) = bot.world().and_then(|w| w.view()) else { return 0 };
    match s.block_runtime_id {
        // Recipe outputs carry no block id: the block of the item's name, in its first state.
        0 => bot.state().item_name(s).and_then(|name| view.world().registry().states_of(name).next()).map_or(0, |(id, _)| id),
        wire => view.world().runtime_id(wire),
    }
}

pub fn stack_of(bot: &Bot, s: &ItemStack) -> Option<Stack> {
    let name = bot.state().item_name(s).filter(|_| !s.is_empty())?.to_owned();
    let banner = crate::item_look::banner(&name, s);
    let map = s.map_id().and_then(|id| bot.state().maps.get(id).cloned());
    Some(Stack { name, aux: s.metadata, count: s.count, block: block_of(bot, s), enchanted: s.is_enchanted(), dye: s.custom_color(), banner, map })
}

/// One entry of the creative inventory: its id, the item, and its tab (an index into acacia-ui's
/// `TABS`).
#[derive(Debug, Clone, PartialEq)]
pub struct CreativeEntry {
    pub id: u32,
    pub stack: Stack,
    pub tab: usize,
}

pub fn creative(bot: &Bot) -> Vec<CreativeEntry> {
    let entry = |i: &acacia_bot::state::CreativeItem| Some(CreativeEntry { id: i.entry_id, stack: stack_of(bot, &i.stack)?, tab: i.tab as usize });
    bot.state().creative.items().iter().filter_map(entry).collect()
}

/// The player list's names, sorted.
pub fn player_names(bot: &Bot) -> Vec<String> {
    let mut names: Vec<String> = bot.state().player_list.iter().map(|p| p.username.clone()).collect();
    names.sort_unstable_by_key(|n| n.to_lowercase());
    names
}

pub fn inventory(bot: &Bot) -> Inventory {
    let state = bot.state();
    let stack = |s: &ItemStack| stack_of(bot, s);
    let inv = &state.inventory;
    let station = bot.open_container().and_then(|c| crate::stations::station(c.window_type));
    Inventory {
        station,
        progress: station.map_or(Progress::default(), |s| crate::stations::progress(s, &state.containers.data)),
        main: inv.main.iter().map(stack).collect(),
        armor: std::array::from_fn(|i| inv.armor.get(i).and_then(stack)),
        offhand: stack(&inv.offhand),
        cursor: stack(inv.cursor()),
        // A station's screen shows as soon as its window opens: its contents may come slot by slot.
        container: bot.open_container().filter(|c| station.is_some() || c.window_type == WindowType::Container && !c.slots.is_empty() && c.slots.len() % 9 == 0).map(|c| Rows {
            title: station.map_or(if c.slots.len() > 27 { "Large Chest" } else { "Chest" }, Station::title).into(),
            slots: (0..c.slots.len().max(station.map_or(0, Station::slot_count))).map(|i| c.slots.get(i).and_then(stack)).collect(),
        }),
        workbench: bot.open_container().filter(|c| c.window_type == WindowType::Workbench).and_then(|c| c.position.as_ref()).map(|p| [p.x, p.y, p.z]),
        ui: inv.ui.iter().map(stack).collect(),
        craftable: Vec::new(),
        crafted: None,
        bench: bot.open_container().and_then(|c| crate::stations::bench(c.window_type)),
        patterns: bot.loom_choices(),
        enchants: bot.enchant_costs(),
        trade: crate::stations::trade(bot),
        picks: bot.stonecutter_choices().into_iter().filter_map(|(id, s)| Some((id, stack_of(bot, &s)?))).collect(),
    }
}

/// An item stack as the HUD shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stack {
    /// `minecraft:apple`.
    pub name: String,
    pub aux: u32,
    pub count: u16,
    /// Block runtime id of a block item, else 0.
    pub block: u32,
    pub enchanted: bool,
    /// Dyed leather's colour.
    pub dye: Option<[u8; 3]>,
    /// A banner's cloth.
    pub banner: Option<std::sync::Arc<acacia_render::banner::Banner>>,
    /// A filled map's picture, once the server has sent it.
    pub map: Option<std::sync::Arc<acacia_bot::state::MapImage>>,
}

/// The text on the side of the sign whose editor the server opened, and whether that sign hangs;
/// `None` without an editor.
pub fn open_sign(bot: &Bot) -> Option<(String, bool)> {
    let editor = bot.state().signs.editor?;
    let entities = &bot.state().block_entities;
    let hanging = entities.sign(editor.position).is_some_and(|nbt| matches!(nbt.value.get("id"), Some(acacia_bot::proto::nbt::Value::String(id)) if &**id == "HangingSign"));
    Some((entities.sign_text(editor.position, editor.front).unwrap_or_default().to_owned(), hanging))
}

/// What a grid or workstation still holds goes back into the inventory first.
async fn close_screen(bot: &mut Bot) -> Result<(), acacia_bot::ActionError> {
    bot.take_back_ui().await?;
    bot.close_container().await
}

/// Item moves wait for the server's answer (one round trip) before the next command runs.
pub async fn apply(bot: &mut Bot, command: Command) {
    tracing::debug!(?command, "command");
    let result = match command {
        Command::Inventory(true) => {
            bot.open_inventory();
            Ok(())
        }
        Command::Inventory(false) => match bot.state().inventory.cursor().is_empty() {
            true => close_screen(bot).await,
            // Vanilla throws what the cursor still holds when the screen closes.
            false => match bot.drop_cursor(false).await {
                Ok(()) => close_screen(bot).await,
                Err(e) => Err(e),
            },
        },
        Command::TakeCrafted { all, name } => bot.take_station_result(all, name.as_deref()).await,
        Command::Name(_) => Ok(()),
        Command::TakeCut { id, all } => bot.take_stonecut(id, all).await,
        Command::OpenBook => {
            bot.open_held_book();
            Ok(())
        }
        Command::EditBook(edit) => bot.edit_book(&edit),
        Command::Loom(pattern) => bot.take_loom(&pattern).await,
        Command::Enchant(option) => bot.take_enchant(option).await,
        Command::Trade(offer) => bot.trade_once(offer).await,
        Command::Creative { id, count } => bot.take_creative(id, count).await,
        Command::Beacon(primary, secondary) => bot.take_beacon(primary, secondary).await,
        Command::Click(slot, click) => bot.click_slot(slot, click).await,
        Command::Craft { name, table } => bot.craft(&name, 1, table).await.map(|_| ()),
        Command::DropCursor { one } => bot.drop_cursor(one).await,
        Command::AnswerForm(id, reply) => bot.answer_form_now(id, reply),
        Command::WriteSign(text) => {
            let text = text.or_else(|| open_sign(bot).map(|(text, _)| text)).unwrap_or_default();
            bot.write_open_sign(&text)
        }
        Command::Controls(c) => {
            if let Some(controls) = bot.controls() {
                // The flight follows the player's own Space taps (`Play::controls`): a changed `fly` would add
                // the bot's double tap, which BDS could pair with the player's.
                *controls = Controls { glide: controls.glide, fly: controls.fly, ..c };
            }
            Ok(())
        }
        Command::Hotbar(slot) => bot.select_hotbar(slot),
        Command::Mine(Some((pos, face))) => bot.start_mining(pos.to_array(), face),
        Command::Mine(None) => {
            bot.stop_mining();
            Ok(())
        }
        Command::UseOn(pos, face) => bot.right_click_block(pos.to_array(), face),
        Command::Entity { runtime_id, attack } => bot.click_entity_aimed(runtime_id, attack),
        Command::UseItem => {
            bot.use_item();
            Ok(())
        }
        Command::ReleaseItem => {
            bot.release_item();
            Ok(())
        }
        Command::Swing => {
            bot.swing();
            Ok(())
        }
        Command::Dismount => bot.dismount().await,
        Command::Respawn => {
            bot.respawn();
            Ok(())
        }
        Command::Chat(text) => {
            let sent = match text.strip_prefix('/') {
                Some(command) => bot.client().command(command),
                None => bot.client().chat(&text),
            };
            if sent { Ok(()) } else { Err(acacia_bot::ActionError::NotPossible("chat not sent".into())) }
        }
    };
    if let Err(e) = result {
        tracing::debug!(%e, "command not carried out");
    }
}
