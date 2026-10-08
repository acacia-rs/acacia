//! The bot thread's side of playing: commands from the window, and the player as each tick leaves it.

use acacia_bot::Bot;
use acacia_bot::interact::Face;
use acacia_bot::movement::Controls;
use acacia_bot::proto::types::GameMode;
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
    Chat(String),
}

/// The player after a tick, for the camera and the HUD.
#[derive(Debug, Clone, PartialEq)]
pub struct Me {
    /// Simulated eye position.
    pub eye: DVec3,
    pub mining: Option<(IVec3, f32)>,
    pub hotbar: u8,
    pub game_mode: GameMode,
    pub health: f32,
    pub max_health: f32,
    pub food: f32,
    pub xp_level: i32,
    pub xp_progress: f32,
    /// The nine hotbar stacks.
    pub items: [Option<Stack>; 9],
}

/// An item stack as the HUD shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stack {
    /// `minecraft:apple`.
    pub name: String,
    pub aux: u32,
    pub count: u16,
}

pub fn apply(bot: &mut Bot, command: Command) {
    let result = match command {
        Command::Controls(c) => {
            if let Some(controls) = bot.controls() {
                *controls = Controls { glide: controls.glide, ..c };
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
    let items = std::array::from_fn(|slot| {
        let stack = state.inventory.main.get(slot).filter(|s| !s.is_empty())?;
        Some(Stack { name: state.item_name(stack)?.to_owned(), aux: stack.metadata, count: stack.count })
    });
    Me {
        eye: DVec3::new(x.into(), y.into(), z.into()),
        mining: bot.mining_progress().map(|(p, f)| (IVec3::from_array(p), f)),
        hotbar: state.inventory.selected_hotbar_slot,
        game_mode: p.game_mode,
        health: p.health,
        max_health: p.max_health,
        food: p.hunger,
        xp_level: p.xp_level,
        xp_progress: p.xp_progress,
        items,
    }
}
