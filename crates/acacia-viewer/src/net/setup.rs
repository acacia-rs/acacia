//! `ACACIA_COMMANDS="summon cow;wait 2;time set day"`: commands sent for unattended shots (the
//! player needs to be an operator). `ACACIA_COMMANDS_AFTER=secs` times the first; a `wait secs`
//! among them holds the rest back that long (a bolt struck just before the shot).

use acacia_bot::Bot;

/// Ticks after spawning before the first goes out: BDS ignored commands sent at once.
const AFTER_TICKS: u32 = 20;

pub struct Setup {
    commands: Vec<String>,
    /// The tick in the world the next command waits for.
    after: u32,
    ticks: u32,
}

impl Setup {
    pub fn from_env() -> Setup {
        let after = std::env::var("ACACIA_COMMANDS_AFTER").ok().and_then(|s| s.parse::<f32>().ok()).map_or(AFTER_TICKS, |s| (s * 20.0) as u32);
        let commands = std::env::var("ACACIA_COMMANDS").iter().flat_map(|s| s.split(';')).map(|c| c.trim().to_owned()).collect();
        Setup { commands, after, ticks: 0 }
    }

    /// Once per bot tick: sends the commands that are due.
    pub fn tick(&mut self, bot: &Bot) {
        if bot.movement().is_some_and(|m| m.is_started()) {
            self.ticks += 1;
        }
        while self.ticks >= self.after && !self.commands.is_empty() {
            let command = self.commands.remove(0);
            tracing::info!(command, "setup");
            match command.strip_prefix("wait ").and_then(|s| s.trim().parse::<f32>().ok()) {
                Some(secs) => self.after = self.ticks + (secs * 20.0) as u32,
                None => _ = bot.client().command(&command),
            }
        }
    }
}
