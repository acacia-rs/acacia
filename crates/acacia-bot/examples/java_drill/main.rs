//! One Java drill on an offline server where the bot is operator (tools/java-drills.sh runs the suite on Paper +
//! Grim): the drill sets itself up by commands, runs its course, then prints
//! `DRILL <name> setbacks <n> check <ok|reason>`; the runner adds Grim's alerts for the same window.
//! `cargo run -p acacia-bot --features java --example java_drill -- <host> <port> <drill> [name]`
mod attrs;
mod camel;
mod crowd;
mod drills;
mod inside;
mod minecart;
mod riptide;
mod sizes;

use std::sync::{Arc, Mutex};

use acacia_bot::{Bot, BotConfig, BotEvent, Events};

/// A drill's place on the flat world (surface y -60), clear of the others.
pub const FLOOR: i32 = -61;
const SETTLE_TICKS: u32 = 40;

pub struct Drill {
    pub bot: Bot,
    pub events: Arc<Mutex<Vec<BotEvent>>>,
    /// Where a drill's second player joins.
    pub server: (String, u16),
}

impl Drill {
    /// Runs `commands`, each a few ticks apart, then lets the setup's teleports land.
    pub async fn setup(&mut self, commands: &[&str]) -> Result<(), String> {
        for c in commands {
            self.bot.command(c);
            self.ticks(3).await?;
        }
        self.ticks(SETTLE_TICKS).await
    }

    /// A failed wait, with the connection's close reason when it closed.
    fn why(&self, e: impl std::fmt::Display) -> String {
        match self.bot.disconnect_reason() {
            Some(reason) => format!("{e}: {reason:?}"),
            None => e.to_string(),
        }
    }

    /// `DRILL_TRACE=1` prints the player after every tick.
    pub async fn ticks(&mut self, n: u32) -> Result<(), String> {
        if std::env::var_os("DRILL_TRACE").is_none() {
            let waited = self.bot.wait_ticks(n).await;
            return waited.map_err(|e| self.why(e));
        }
        for _ in 0..n {
            let waited = self.bot.wait_ticks(1).await;
            waited.map_err(|e| self.why(e))?;
            if let Some(j) = self.bot.java() {
                let p = &j.client().movement.player;
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                let (s, ms) = (now.as_secs() % 86_400, now.subsec_millis());
                let clock = format!("{:02}:{:02}:{:02}.{ms:03}", s / 3600, s / 60 % 60, s % 60);
                println!(
                    "tick {clock} {:.5?} delta {:.5?} ground {} setbacks {} keys {:?} effects {:?}",
                    p.pos, p.delta, p.on_ground, j.setbacks(), p.keys, p.effects
                );
            }
        }
        Ok(())
    }

    /// Holds `forward` (and jump, sprint) for `n` ticks.
    pub async fn walk(&mut self, n: u32, forward: f32, jump: bool, sprint: bool) -> Result<(), String> {
        for _ in 0..n {
            if let Some(c) = self.bot.controls() {
                (c.forward, c.jump, c.sprint) = (forward, jump, sprint);
            }
            self.ticks(1).await?;
        }
        if let Some(c) = self.bot.controls() {
            (c.forward, c.jump, c.sprint) = (0.0, false, false);
        }
        Ok(())
    }

    pub fn feet(&self) -> [f64; 3] {
        self.bot.java().map_or([0.0; 3], |j| j.client().movement.player.pos)
    }

    pub fn setbacks(&self) -> u32 {
        self.bot.java().map_or(0, |j| j.setbacks())
    }

    pub fn seen(&self, f: impl Fn(&BotEvent) -> bool) -> bool {
        self.events.lock().unwrap().iter().any(f)
    }
}

impl Drill {
    /// Joins `server` as `name` and waits for the join's teleports to settle.
    pub async fn connect(server: &(String, u16), name: &str) -> Result<Self, String> {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let mut config = BotConfig::default();
        config.events = Events::ALL;
        config.trackers.entities = true;
        config.auto_respawn = true;
        config.event_sink = Some(Arc::new(move |e| sink.lock().unwrap().push(e)));
        let bot = Bot::connect_java_offline(&server.0, server.1, name, config).await.map_err(|e| e.to_string())?;
        let mut d = Drill { bot, events, server: server.clone() };
        d.ticks(SETTLE_TICKS).await?;
        Ok(d)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port, drill] = &args[..3] else { return Err("usage: java_drill <host> <port> <drill> [name]".into()) };
    let name = args.get(3).map_or("Driller", String::as_str);
    let mut d = Drill::connect(&(host.clone(), port.parse()?), name).await?;
    let (start, check) = match drills::run(drill, &mut d).await {
        Ok((start, check)) => (start, check),
        Err(e) => (d.setbacks(), Err(e)),
    };
    let check = check.err().unwrap_or_else(|| "ok".into());
    println!("DRILL {drill} setbacks {} check {check}", d.setbacks() - start);
    d.bot.disconnect().await;
    Ok(())
}
