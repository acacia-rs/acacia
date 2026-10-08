//! The sidebar objective as the window shows it: player entries named from the player list.

use acacia_bot::Bot;
use acacia_ui::sidebar::Sidebar;

/// A player's entry while they are not in the player list (`commands.scoreboard.players.offlinePlayerName`).
const OFFLINE: &str = "Player Offline";

pub fn sidebar(bot: &Bot) -> Option<Sidebar> {
    let state = bot.state();
    let objective = state.scoreboard.sidebar()?;
    let name = |id: Option<i64>| id.and_then(|id| state.player_list.iter().find(|p| p.entity_unique_id == id)).map(|p| p.username.clone());
    let lines = objective
        .sorted()
        .into_iter()
        .map(|e| {
            let shown = if e.display.is_empty() { name(e.entity_unique_id).unwrap_or_else(|| OFFLINE.to_owned()) } else { e.display.clone() };
            (shown, e.score)
        })
        .collect();
    Some(Sidebar { title: objective.display_name.clone(), lines })
}
