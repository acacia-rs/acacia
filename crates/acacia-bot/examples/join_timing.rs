//! How long a join takes, for links made worse with tools/netem: from the first packet to the
//! spawn, to every column of the view (a circle of `radius` around the spawn column, as servers
//! send it) being known, and to the last section answer before `quiet` ms of none. One line:
//! `join spawn_ms=.. columns_ms=.. sections_ms=.. columns=have/want subchunks=n`.
//! `cargo run --release -p acacia-bot --example join_timing -- <server> [name] [radius] [quiet ms] [limit s]`
use std::time::{Duration, Instant};

use acacia_bot::client::proto::Packet;
use acacia_bot::client::proto::packets::Subchunk;
use acacia_bot::client::{Client, PacketFilter};
use acacia_bot::{Bot, BotConfig, BotEvent};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "Timer".into());
    let radius: i32 = args.next().map_or(Ok(8), |s| s.parse())?;
    let quiet = Duration::from_millis(args.next().map_or(Ok(3000), |s| s.parse())?);
    let limit = Duration::from_secs(args.next().map_or(Ok(120), |s| s.parse())?);

    let start = Instant::now();
    let builder = Client::builder(&server).offline(&name).chunk_radius(radius).login_timeout(limit);
    let config = BotConfig { physics: true, subscribe: PacketFilter::none().with(Subchunk::ID), ..BotConfig::default() };
    let mut bot = Bot::connect(builder, config).await?;
    let spawned = start.elapsed();
    let want: Vec<[i32; 2]> = (-radius..=radius).flat_map(|dx| (-radius..=radius).map(move |dz| [dx, dz])).filter(|[dx, dz]| dx * dx + dz * dz <= radius * radius).collect();
    let (mut columns_at, mut last_section, mut subchunks, mut have) = (None, start, 0usize, 0usize);
    let mut poll = tokio::time::interval(Duration::from_millis(20));
    let outcome = loop {
        tokio::select! {
            event = bot.next() => match event {
                Some(BotEvent::Packet(packet)) if packet.id == Subchunk::ID => {
                    subchunks += 1;
                    last_section = Instant::now();
                }
                Some(BotEvent::Disconnected(reason)) => break format!("disconnected {reason:?}"),
                None => break "closed".to_owned(),
                Some(_) => {}
            },
            _ = poll.tick() => {
                let centre = bot.movement().and_then(|movement| movement.position()).map(|[x, _, z]| [(x.floor() as i32) >> 4, (z.floor() as i32) >> 4]);
                if let (Some([cx, cz]), Some(view)) = (centre, bot.world().and_then(|world| world.view())) {
                    have = want.iter().filter(|[dx, dz]| view.chunk(cx + dx, cz + dz).is_some()).count();
                    if have == want.len() && columns_at.is_none() {
                        columns_at = Some(start.elapsed());
                    }
                }
                let settled = columns_at.is_some() && last_section.max(start + columns_at.unwrap_or_default()).elapsed() >= quiet;
                if settled {
                    break "ok".to_owned();
                }
                if start.elapsed() >= limit {
                    break "timeout".to_owned();
                }
            }
        }
    };
    let ms = |at: Option<Duration>| at.map_or("-".to_owned(), |at| at.as_millis().to_string());
    let sections = (subchunks > 0).then(|| last_section - start);
    println!("join {outcome} spawn_ms={} columns_ms={} sections_ms={} columns={have}/{} subchunks={subchunks}", spawned.as_millis(), ms(columns_at), ms(sections), want.len());
    bot.client().close();
    Ok(())
}
