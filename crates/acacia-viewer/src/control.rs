//! The bot thread's side of playing: commands from the window, and the player as each tick leaves it.

use acacia_bot::Bot;
use acacia_bot::forms::FormReply;
use acacia_bot::interact::Face;
use acacia_bot::items::{Click, SlotRef};
use acacia_bot::state::ItemStack;
use acacia_bot::movement::Controls;
use acacia_bot::proto::packets::BossEventColor;
use acacia_bot::proto::types::{GameMode, WindowType};
use glam::{DVec3, IVec3};

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
    /// The death screen's respawn button.
    Respawn,
    Chat(String),
    /// The inventory screen opened (E) or closed.
    Inventory(bool),
    /// A click on a slot of the open screen.
    Click(SlotRef, Click),
    /// A recipe-book click: one craft of item `name`, at the crafting table at `table` if given.
    Craft { name: String, table: Option<[i32; 3]> },
    /// A click outside the screen: throws the held stack, or one of it.
    DropCursor { one: bool },
    /// The player's answer to open form `id`.
    AnswerForm(u32, FormReply),
}

/// The player's own slots as a screen shows them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Inventory {
    /// 0-8 the hotbar.
    pub main: Vec<Option<Stack>>,
    pub armor: [Option<Stack>; 4],
    pub offhand: Option<Stack>,
    pub cursor: Option<Stack>,
    /// The open container, when it is rows of nine (chests, barrels, shulker boxes).
    pub container: Option<Rows>,
    /// An open crafting table's position.
    pub workbench: Option<[i32; 3]>,
    /// What can be crafted now (in the 2x2 grid, or at the open table); filled by [`craftable`].
    pub craftable: Vec<Stack>,
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

fn stack_of(bot: &Bot, s: &ItemStack) -> Option<Stack> {
    let name = bot.state().item_name(s).filter(|_| !s.is_empty())?.to_owned();
    Some(Stack { name, aux: s.metadata, count: s.count, block: block_of(bot, s) })
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
    Inventory {
        main: inv.main.iter().map(stack).collect(),
        armor: std::array::from_fn(|i| inv.armor.get(i).and_then(stack)),
        offhand: stack(&inv.offhand),
        cursor: stack(inv.cursor()),
        container: bot.open_container().filter(|c| c.window_type == WindowType::Container && !c.slots.is_empty() && c.slots.len() % 9 == 0).map(|c| Rows {
            title: if c.slots.len() > 27 { "Large Chest" } else { "Chest" }.into(),
            slots: c.slots.iter().map(stack).collect(),
        }),
        workbench: bot.open_container().filter(|c| c.window_type == WindowType::Workbench).and_then(|c| c.position.as_ref()).map(|p| [p.x, p.y, p.z]),
        craftable: Vec::new(),
    }
}

/// The player after a tick, for the camera and the HUD.
#[derive(Debug, Clone, PartialEq)]
pub struct Me {
    /// Simulated eye position.
    pub eye: DVec3,
    pub mining: Option<(IVec3, f32)>,
    pub hotbar: u8,
    pub game_mode: GameMode,
    pub alive: bool,
    /// How hard it rains, 0 to 1.
    pub rain: f32,
    pub health: f32,
    pub max_health: f32,
    pub food: f32,
    pub xp_level: i32,
    pub xp_progress: f32,
    /// The nine hotbar stacks.
    pub items: [Option<Stack>; 9],
    /// Boss bars: name, fill 0 to 1, and an index of `acacia_ui::overlay::BOSS_COLOURS`.
    pub bosses: Vec<(String, f32, usize)>,
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
            true => bot.close_container().await,
            // Vanilla throws what the cursor still holds when the screen closes.
            false => match bot.drop_cursor(false).await {
                Ok(()) => bot.close_container().await,
                Err(e) => Err(e),
            },
        },
        Command::Click(slot, click) => bot.click_slot(slot, click).await,
        Command::Craft { name, table } => bot.craft(&name, 1, table).await.map(|_| ()),
        Command::DropCursor { one } => bot.drop_cursor(one).await,
        Command::AnswerForm(id, reply) => bot.answer_form_now(id, reply),
        Command::Controls(c) => {
            if let Some(controls) = bot.controls() {
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

pub fn me(bot: &Bot) -> Me {
    let [x, y, z] = bot.eye_position();
    let state = bot.state();
    let p = &state.player;
    let items = std::array::from_fn(|slot| stack_of(bot, state.inventory.main.get(slot)?));
    Me {
        eye: DVec3::new(x.into(), y.into(), z.into()),
        mining: bot.mining_progress().map(|(p, f)| (IVec3::from_array(p), f)),
        hotbar: state.inventory.selected_hotbar_slot,
        game_mode: p.game_mode,
        alive: p.alive,
        rain: state.environment.rain,
        health: p.health,
        max_health: p.max_health,
        food: p.hunger,
        xp_level: p.xp_level,
        xp_progress: p.xp_progress,
        items,
        bosses: state.environment.boss_bars.values().map(|b| (b.title.clone(), b.progress, boss_colour(b.color))).collect(),
    }
}

fn boss_colour(colour: BossEventColor) -> usize {
    use BossEventColor as C;
    match colour {
        C::Blue => 1,
        C::Red => 2,
        C::Green => 3,
        C::Yellow => 4,
        C::Purple | C::RebeccaPurple => 5,
        C::White => 6,
        C::Pink | C::Unknown(_) => 0,
    }
}
