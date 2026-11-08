//! Walks a key script on an offline server for the anticheat gate (tools/grim-gate.sh):
//! `java_gate <host> <port> <settle_seconds> <glider:0|1> [steps]`. Steps are xkeys.py's: `w=3`, `Control_L+w=2`,
//! `wait=1`, `down=w`, `up=w`, `look=dx,dy`; mouse and hotbar steps only pass the time.
use std::time::Duration;

use acacia_java_client::Client;
use acacia_java_physics::Keys;
use acacia_java_proto::ids;
use tokio::time::{Instant, timeout_at};

const TICK: Duration = Duration::from_millis(50);
const LOOK_TICKS: u32 = 10;
/// Degrees per pixel at the default sensitivity.
const DEGREES_PER_PIXEL: f32 = 0.15;

fn press(keys: &mut Keys, name: &str, on: bool) {
    match name {
        "w" => keys.forward = on,
        "s" => keys.backward = on,
        "a" => keys.left = on,
        "d" => keys.right = on,
        "space" => keys.jump = on,
        "Shift_L" => keys.shift = on,
        "Control_L" => keys.sprint = on,
        _ => {}
    }
}

#[derive(Clone, Copy)]
struct Tick {
    keys: Keys,
    turn: [f32; 2],
}

fn expand(steps: &[String]) -> Vec<Tick> {
    let mut held = Keys::default();
    let mut ticks = Vec::new();
    for step in steps {
        let (name, value) = step.split_once('=').expect("step is name=value");
        let seconds = value.parse::<f32>().ok();
        let count = seconds.map_or(1, |s| ((s * 20.0).round() as usize).max(1));
        match name {
            "down" => press(&mut held, value, true),
            "up" => press(&mut held, value, false),
            "look" => {
                let (dx, dy) = value.split_once(',').expect("look=dx,dy");
                let per_tick = |pixels: &str| pixels.parse::<f32>().expect("pixels") * DEGREES_PER_PIXEL / LOOK_TICKS as f32;
                ticks.extend(std::iter::repeat_n(Tick { keys: held, turn: [per_tick(dx), per_tick(dy)] }, LOOK_TICKS as usize));
            }
            _ => {
                let mut keys = held;
                name.split('+').for_each(|k| press(&mut keys, k, true));
                ticks.extend(std::iter::repeat_n(Tick { keys, turn: [0.0; 2] }, count));
            }
        }
    }
    ticks
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let settle = (args[2].parse::<f32>()? * 20.0) as usize;
    let cheat = std::env::var_os("ACACIA_GATE_CHEAT").is_some();
    let mut client = Client::join_offline(&args[0], args[1].parse()?, "AcaciaGate").await?;
    client.movement.player.has_glider = args[3] == "1";
    let mut script = vec![Tick { keys: Keys::default(), turn: [0.0; 2] }; settle];
    script.extend(expand(&args[4..]));
    let (mut corrections, total) = (0, script.len());
    let mut next = Instant::now() + TICK;
    let mut at = 0;
    while at < total {
        match timeout_at(next, client.readable()).await {
            Ok(ready) => {
                ready?;
                let packet = client.next().await?;
                if packet.id == ids::play::clientbound::PLAYER_POSITION && at > settle {
                    corrections += 1;
                    println!("server moved the bot at tick {at} to {:?}", client.pose.position);
                }
            }
            Err(_) => {
                let tick = script[at];
                let player = &mut client.movement.player;
                player.yaw += tick.turn[0];
                player.pitch = (player.pitch + tick.turn[1]).clamp(-90.0, 90.0);
                client.step(tick.keys).await?;
                // Control run: proves the gate can fail.
                if cheat && at > settle {
                    let delta = &mut client.movement.player.delta;
                    delta[0] *= 1.3;
                    delta[2] *= 1.3;
                }
                at += 1;
                // No catch-up bursts after a stall: Grim's TimerLimit counts them.
                next = (next + TICK).max(Instant::now());
            }
        }
    }
    println!("finished at {:?} after {total} ticks, {corrections} server corrections", client.pose.position);
    Ok(())
}
