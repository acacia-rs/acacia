//! Shared harness for the movement examples (drills, fuzz): a recording physics bot on a server where it
//! is operator, and a floating stone pad to build test terrain on.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use acacia_bot::client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_bot::client::Client;
use acacia_bot::world::SharedWorlds;
use acacia_bot::{Bot, BotConfig};
use tokio::sync::mpsc;

pub type Error = Box<dyn std::error::Error>;
/// Fill from/to (offsets from the pad origin) with a block.
pub type Fill<'a> = ([i32; 3], [i32; 3], &'a str);

/// The pad floats this far above the bot's spawn, clear of terrain and stray water.
const PAD_HEIGHT: i32 = 40;
/// Pad extent (offsets from the origin); the floor is the five layers below y 0.
pub const PAD_MIN: [i32; 3] = [-3, -5, -12];
pub const PAD_MAX: [i32; 3] = [24, 10, 12];

/// Connects `name` (`@account` for online mode) with physics on, recording to `BEDROCK_RECORD` if set,
/// waits for movement to start and clears mobs (their hits look like physics mismatches).
pub async fn connect(server: &str, name: &str) -> Result<(Bot, Pad), Error> {
    let record = std::env::var_os("BEDROCK_RECORD").map(Into::into);
    connect_with(server, name, record, SharedWorlds::new()).await
}

/// [`connect`] recording to `record`, sharing terrain with other bots through `worlds`.
pub async fn connect_with(server: &str, name: &str, record: Option<PathBuf>, worlds: SharedWorlds) -> Result<(Bot, Pad), Error> {
    let mut builder = Client::builder(server);
    builder = match name.strip_prefix('@') {
        Some(account) => {
            let account = Account::new(Arc::new(AuthClient::new(AuthConfig::default())?), Arc::new(FileTokenCache::new(".tokens")?), account);
            let (key, credentials) = account.login_credentials().await?;
            builder.online(credentials, key)
        }
        None => builder.offline(name),
    };
    let config = BotConfig { physics: true, record, shared_worlds: worlds, ..BotConfig::default() };
    let mut bot = Bot::connect(builder, config).await?;
    while bot.movement().is_none_or(|m| !m.is_started()) {
        bot.wait_ticks(1).await?;
    }
    bot.wait_ticks(20).await?;
    let spawn = bot.block_position().expect("movement started");
    println!("pad origin {spawn:?} + {PAD_HEIGHT} up");
    for cmd in ["/difficulty peaceful", "/kill @e[type=!player]"] {
        bot.client().command(cmd);
        bot.wait_ticks(2).await?;
    }
    Ok((bot, Pad { spawn, commander: Commander::Own }))
}

pub fn corrections(bot: &Bot) -> u32 {
    bot.movement().map_or(0, |m| m.corrections)
}

/// Who runs a pad's commands: the bot itself, or an operator they are relayed to with `@s` replaced by
/// the bot's name (offline players are not operators, and BDS only ops from its console).
pub enum Commander {
    Own,
    Relay(mpsc::UnboundedSender<String>, String),
}

pub struct Pad {
    spawn: [i32; 3],
    commander: Commander,
}

impl Pad {
    /// A pad whose floor top is `PAD_HEIGHT` above `origin`, wherever the bot spawned: offline players
    /// rejoin where they left, so a spawn-relative pad would creep upwards run after run.
    pub fn at(self, origin: [i32; 3]) -> Pad {
        Pad { spawn: origin, ..self }
    }

    /// The same pad moved `dx` blocks along x (one pad per bot when several share a server).
    pub fn shifted(self, dx: i32) -> Pad {
        let [x, y, z] = self.spawn;
        Pad { spawn: [x + dx, y, z], ..self }
    }

    pub fn relayed(self, commander: Commander) -> Pad {
        Pad { commander, ..self }
    }

    fn run(&self, bot: &Bot, command: &str) {
        match &self.commander {
            Commander::Own => {
                bot.client().command(command);
            }
            Commander::Relay(to, name) => {
                let _ = to.send(command.replace("@s", name));
            }
        }
    }

    /// World block position of a pad offset.
    pub fn block(&self, [dx, dy, dz]: [i32; 3]) -> [i32; 3] {
        let [x, y, z] = self.spawn;
        [x + dx, y + PAD_HEIGHT + dy, z + dz]
    }

    /// Rebuilds the bare pad, places `fills` in order and waits `settle` ticks (flowing liquids).
    pub async fn build(&self, bot: &mut Bot, fills: &[Fill<'_>], settle: u32) -> Result<(), Error> {
        // Park the bot beside the pad first, so it is never inside blocks while they are refilled.
        let mut cmds = vec![self.fill([-8, -1, 0], [-8, -1, 0], "stone")];
        cmds.push(self.tp_command([-7.5, 0.0, 0.5], -90.0));
        cmds.extend([self.fill([PAD_MIN[0], 0, PAD_MIN[2]], PAD_MAX, "air"), self.fill(PAD_MIN, [PAD_MAX[0], -1, PAD_MAX[2]], "stone")]);
        cmds.extend(fills.iter().map(|&(a, b, block)| self.fill(a, b, block)));
        for cmd in cmds {
            self.run(bot, &cmd);
            bot.wait_ticks(2).await?;
        }
        Ok(bot.wait_ticks(settle).await?)
    }

    /// Teleports to `at` (feet, offset from the origin block's corner) and waits until it lands: BDS can
    /// send the teleport many ticks late after big fills.
    pub async fn teleport(&self, bot: &mut Bot, at: [f32; 3], yaw: f32) -> Result<(), Error> {
        let teleports = bot.movement().map_or(0, |m| m.teleports);
        self.run(bot, &self.tp_command(at, yaw));
        for _ in 0..100 {
            if bot.movement().is_some_and(|m| m.teleports > teleports) {
                break;
            }
            bot.wait_ticks(1).await?;
        }
        Ok(bot.wait_ticks(2).await?)
    }

    /// Back to the spawn block, or the next run's pad would float another `PAD_HEIGHT` higher.
    pub async fn go_home(&self, bot: &mut Bot) -> Result<(), Error> {
        let [x, y, z] = self.spawn;
        self.run(bot, &format!("/tp @s {} {y} {}", x as f32 + 0.5, z as f32 + 0.5));
        Ok(bot.wait_ticks(10).await?)
    }

    fn fill(&self, a: [i32; 3], b: [i32; 3], block: &str) -> String {
        let ([ax, ay, az], [bx, by, bz]) = (self.block(a), self.block(b));
        format!("/fill {ax} {ay} {az} {bx} {by} {bz} {block}")
    }

    fn tp_command(&self, at: [f32; 3], yaw: f32) -> String {
        let [x, y, z] = self.block([0, 0, 0]).map(|v| v as f32);
        // Formatted from floats: "{x}.5" with a negative x would land in the neighbouring block.
        format!("/tp @s {} {} {} {yaw} 0", x + at[0], y + at[1], z + at[2])
    }
}
