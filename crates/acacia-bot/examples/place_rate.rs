//! Live probe of BDS's placement limits (before pipelining placements): from one spot, unconfirmed clicks onto
//! the pad floor at 1, 2, 4 and 8 per tick along a row, clicks out of reach, and a click while looking away. An
//! observer then reads what the server kept. Needs the test pack off.
//! `cargo run --release -p acacia-bot --example place_rate -- <server> <name> <observer>`
mod support;

use acacia_bot::interact::Face;
use acacia_bot::survival::Destination;
use acacia_bot::Bot;
use support::{Error, Pad};

const BLOCK: &str = "minecraft:cobblestone";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19170".into());
    let name = args.next().unwrap_or_else(|| "Prober".into());
    let observer = args.next().unwrap_or_else(|| "ProbeObs".into());
    let (mut a, pad) = support::connect(&server, &name).await?;
    pad.build(&mut a, &[], 5).await?;
    for c in ["/clear @s", "/give @s cobblestone 64", "/give @s cobblestone 64"] {
        pad.run(&a, c);
        a.wait_ticks(4).await?;
    }
    pad.teleport(&mut a, [0.5, 0.0, 0.5], 0.0).await?;
    let slot = a.find_item_named(BLOCK).ok_or("no cobblestone")?;
    a.equip(slot, Destination::Hand).await?;
    a.wait_ticks(5).await?;

    // (label, cells, clicks per tick, look at the row first)
    // Disjoint cells: a click into an already filled cell must not count as kept.
    let row = |z: i32| (-2..=3).map(|x| [x, 0, z]).collect::<Vec<_>>();
    let rows: Vec<(String, Vec<[i32; 3]>, usize, bool)> = vec![
        ("3/tick".into(), row(2), 3, true),
        ("4/tick".into(), row(-2), 4, true),
        ("5/tick".into(), row(3), 5, true),
        ("6/tick".into(), row(-3), 6, true),
        ("reach 4".into(), vec![[0, 0, 5]], 1, true),
        ("reach 5".into(), vec![[0, 0, 6]], 1, true),
        ("reach 6".into(), vec![[0, 0, 7]], 1, true),
        ("look away".into(), vec![[0, 0, -5]], 1, false),
    ];
    let mut report = Vec::new();
    for (label, cells, per_tick, look) in &rows {
        let first = pad.block(cells[0]);
        if *look {
            a.look_at([first[0] as f32 + 0.5, first[1] as f32, first[2] as f32 + 0.5]).await?;
        } else {
            a.look_at([0.5 + pad.block([0, 0, 0])[0] as f32, pad.block([0, 1, 0])[1] as f32, pad.block([0, 0, 9])[2] as f32]).await?;
        }
        for chunk in cells.chunks(*per_tick) {
            for c in chunk {
                let floor = pad.block([c[0], c[1] - 1, c[2]]);
                a.right_click_block(floor, Face::Up)?;
            }
            a.wait_ticks(1).await?;
        }
        a.wait_ticks(20).await?;
        report.push((label.clone(), cells.clone()));
    }
    pad.go_home(&mut a).await?;
    a.disconnect().await;
    let (mut o, _) = support::connect(&server, &observer).await?;
    let spot = pad.block([0, 6, 0]);
    pad.run(&o, &format!("/tp @s {} {} {}", spot[0] as f32 + 0.5, spot[1], spot[2] as f32 + 0.5));
    o.wait_ticks(60).await?;
    for (label, cells) in report {
        let kept = cells.iter().filter(|c| placed(&o, &pad, **c)).count();
        println!("{label}: {kept}/{} kept", cells.len());
    }
    o.disconnect().await;
    Ok(())
}

fn placed(o: &Bot, pad: &Pad, cell: [i32; 3]) -> bool {
    o.block_state(pad.block(cell)).is_some_and(|s| s.name == BLOCK)
}
