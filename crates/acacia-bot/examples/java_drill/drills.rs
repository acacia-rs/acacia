//! The drills: each sets itself up in its own strip (x from its index × 32), takes the setback baseline, runs its
//! course and checks what it can see itself. Strictness (0 alerts, 0 setbacks) is the runner's.

use acacia_bot::BotEvent;

use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["speed", "slowness", "jump_boost", "levitation", "slow_falling", "hunger", "powder_snow", "nether_lava", "push", "death", "chat"];

type Outcome = Result<(u32, Result<(), String>), String>;

/// The drill's strip: cleared, the bot alone on it at the west end facing east, no effects or items.
async fn start(d: &mut Drill, x: i32, extra: &[&str]) -> Result<u32, String> {
    let surface = FLOOR + 1;
    let base = [
        "difficulty peaceful".to_owned(),
        "gamemode survival @s".to_owned(),
        "effect clear @s".to_owned(),
        "clear @s".to_owned(),
        format!("kill @e[type=!player,x={x},y={surface},z=0,distance=..40]"),
        format!("fill {} {surface} -4 {} {} 4 air", x - 2, x + 30, surface + 12),
        format!("fill {} {FLOOR} -4 {} {FLOOR} 4 grass_block", x - 2, x + 30),
        format!("execute in minecraft:overworld run tp @s {x} {surface} 0 -90 0"),
    ];
    let commands: Vec<&str> = base.iter().map(String::as_str).chain(extra.iter().copied()).collect();
    d.setup(&commands).await?;
    Ok(d.setbacks())
}

fn check(ok: bool, what: impl FnOnce() -> String) -> Result<(), String> {
    if ok { Ok(()) } else { Err(what()) }
}

pub async fn run(name: &str, d: &mut Drill) -> Outcome {
    let index = NAMES.iter().position(|n| *n == name).ok_or(format!("no drill {name}; drills: {}", NAMES.join(" ")))?;
    let x = 64 + 32 * index as i32;
    let x0 = f64::from(x) + 0.5;
    match name {
        "speed" | "slowness" => {
            let effect = if name == "speed" { "effect give @s speed 60 1 true" } else { "effect give @s slowness 60 2 true" };
            let base = start(d, x, &[effect]).await?;
            d.walk(60, 1.0, false, true).await?;
            d.walk(20, 1.0, true, true).await?;
            let moved = d.feet()[0] - x0;
            // Plain sprinting covers ~24 blocks here; Speed II ~33, Slowness III ~15.
            let ok = if name == "speed" { moved > 28.0 } else { (1.0..19.0).contains(&moved) };
            Ok((base, check(ok, || format!("moved {moved:.2}"))))
        }
        "jump_boost" => {
            let base = start(d, x, &["effect give @s jump_boost 60 2 true"]).await?;
            let top = track_top(d, 60, true).await?;
            Ok((base, check(top > f64::from(FLOOR) + 3.4, || format!("top {top:.2}"))))
        }
        "levitation" => {
            let base = start(d, x, &["effect give @s levitation 2 1 true"]).await?;
            let top = track_top(d, 40, false).await?;
            d.ticks(80).await?;
            Ok((base, check(top > f64::from(FLOOR) + 3.0, || format!("top {top:.2}"))))
        }
        "slow_falling" => {
            // 20 ticks of a plain fall drop ~7 blocks; slow falling's gravity of 0.01 drops ~2.
            let top = FLOOR + 21;
            let (ledge, up) = (format!("setblock {x} {} 0 glass", top - 1), format!("tp @s {x} {top} 0"));
            let base = start(d, x, &[&ledge, &up, "effect give @s slow_falling 30 0 true"]).await?;
            d.bot.command(&format!("setblock {x} {} 0 air", top - 1));
            d.ticks(20).await?;
            let fell = f64::from(top) - d.feet()[1];
            d.ticks(100).await?;
            Ok((base, check(fell < 3.5, || format!("fell {fell:.2} in 20 ticks"))))
        }
        "hunger" => {
            let base = start(d, x, &["difficulty easy", "effect give @s hunger 30 255 true"]).await?;
            for _ in 0..600 {
                if d.bot.state().player.hunger <= 6.0 {
                    break;
                }
                d.ticks(1).await?;
            }
            let food = d.bot.state().player.hunger;
            let mut sprinted = false;
            for _ in 0..60 {
                d.walk(1, 1.0, false, true).await?;
                sprinted |= d.bot.java().is_some_and(|j| j.client().movement.player.sprinting);
            }
            d.bot.command("difficulty peaceful");
            Ok((base, check(food <= 6.0 && !sprinted, || format!("food {food}, sprinted {sprinted}"))))
        }
        "powder_snow" => {
            let snow = format!("fill {} {FLOOR} -1 {} {FLOOR} 1 powder_snow", x + 2, x + 14);
            let base = start(d, x, &["item replace entity @s armor.feet with leather_boots", &snow]).await?;
            let low = track_low(d, 60).await?;
            Ok((base, check(low > f64::from(FLOOR) + 0.99, || format!("sank to {low:.3}"))))
        }
        "nether_lava" => {
            let nether = |c: String| format!("execute in minecraft:the_nether run {c}");
            let setup = [
                nether(format!("forceload add {} -16 {} 16", x - 16, x + 32)),
                nether(format!("fill {} 60 -4 {} 70 4 air", x - 3, x + 20)),
                nether(format!("fill {} 59 -4 {} 59 4 obsidian", x - 3, x + 20)),
                nether(format!("fill {} 60 -1 {} 60 1 lava", x + 2, x + 16)),
                "effect give @s fire_resistance 120 0 true".to_owned(),
                nether(format!("tp @s {x} 60 0 -90 0")),
            ];
            let setup: Vec<&str> = setup.iter().map(String::as_str).collect();
            let base = start(d, x, &setup).await?;
            // The spreading lava's current pushes west harder than walking east gains; Grim judges each tick.
            d.ticks(40).await?;
            d.walk(80, 1.0, false, false).await?;
            let fast = d.bot.java().is_some_and(|j| j.client().movement.player.fast_lava);
            Ok((base, check(fast, || "fast_lava off in the Nether".into())))
        }
        "push" => {
            let cow = format!("summon minecraft:cow {}.8 {} 0.5 {{NoAI:1b}}", x, FLOOR + 1);
            let base = start(d, x, &[]).await?;
            d.bot.command(&cow);
            d.ticks(40).await?;
            let shoved = (d.feet()[0] - x0).abs() + (d.feet()[2] - 0.5).abs();
            d.walk(40, 1.0, false, false).await?;
            Ok((base, check(shoved > 0.1, || format!("pushed {shoved:.3}"))))
        }
        "death" => {
            let base = start(d, x, &[]).await?;
            d.bot.command("kill @s");
            d.ticks(100).await?;
            d.walk(30, 1.0, false, false).await?;
            let (died, back) = (d.seen(|e| matches!(e, BotEvent::Died)), d.seen(|e| matches!(e, BotEvent::Respawned)));
            let alive = d.bot.state().player.alive;
            Ok((base, check(died && back && alive, || format!("died {died}, respawned {back}, alive {alive}"))))
        }
        "chat" => {
            let base = start(d, x, &[]).await?;
            d.bot.command("say drill-say");
            d.bot.command("tellraw @s \"drill-tell\"");
            d.ticks(20).await?;
            let said = |text: &str| d.seen(|e| matches!(e, BotEvent::Chat(m) if m.message.contains(text)));
            let (say, tell) = (said("drill-say"), said("drill-tell"));
            Ok((base, check(say && tell, || format!("say {say}, tellraw {tell}"))))
        }
        _ => unreachable!(),
    }
}

/// The highest feet over `n` ticks, holding forward (and jump).
async fn track_top(d: &mut Drill, n: u32, jump: bool) -> Result<f64, String> {
    let mut top = f64::MIN;
    for _ in 0..n {
        d.walk(1, if jump { 1.0 } else { 0.0 }, jump, false).await?;
        top = top.max(d.feet()[1]);
    }
    Ok(top)
}

/// The lowest feet over `n` ticks walking forward.
async fn track_low(d: &mut Drill, n: u32) -> Result<f64, String> {
    let mut low = f64::MAX;
    for _ in 0..n {
        d.walk(1, 1.0, false, false).await?;
        low = low.min(d.feet()[1]);
    }
    Ok(low)
}
