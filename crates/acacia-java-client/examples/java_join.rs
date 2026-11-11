//! Joins an offline-mode server and checks the decoded world: `java_join [host] [port] [seconds]`.
use std::time::{Duration, Instant};

use acacia_java_client::Client;
use acacia_java_proto::chunk::{read_chunk_header, read_sections};
use acacia_java_proto::ids;
use acacia_registry::BlockState;

fn name(state: Option<u32>) -> String {
    state.and_then(BlockState::from_java_id).map_or("unloaded".into(), |s| s.block().name().to_string())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let host = args.next().unwrap_or_else(|| "127.0.0.1".into());
    let port = args.next().map_or(Ok(25565), |p| p.parse())?;
    let seconds = args.next().map_or(Ok(45), |s| s.parse())?;
    let mut client = Client::join_offline(&host, port, "AcaciaJava").await?;
    println!("joined as {} {:032x}", client.name, client.uuid);
    let end = Instant::now() + Duration::from_secs(seconds);
    let not_air = |id: u32| !BlockState::from_java_id(id).is_some_and(BlockState::is_air);
    let (mut sections, mut wrong) = (0, 0);
    while let Ok(packet) = tokio::time::timeout_at(end.into(), client.next()).await {
        let packet = packet?;
        // At decode time: later block updates change a section without changing the server's count.
        if packet.id == ids::play::clientbound::LEVEL_CHUNK_WITH_LIGHT {
            let (_, _, buffer) = read_chunk_header(&mut &packet.body[..])?;
            for s in read_sections(&buffer)? {
                sections += 1;
                wrong += usize::from(s.count_where(not_air) != s.non_empty as usize);
            }
        }
    }
    let world = &client.world;
    let [x, y, z] = client.pose.position.map(|v| v.floor() as i32);
    println!("{} at {:?}: {} chunks, {sections} sections, {wrong} with a wrong block count, {} entities", world.dimension, client.pose.position, world.chunk_count(), world.entities.len());
    println!("probe set={} fill={} other_fill={}", name(world.block([x, y + 3, z])), name(world.block([x + 1, y + 3, z + 1])), name(world.block([x + 2, y + 3, z + 2])));
    let moved = world.entities.values().filter(|e| e.position.iter().all(|v| v.is_finite()) && (-64.0..400.0).contains(&e.position[1])).count();
    println!("entities with sane positions: {moved}");
    Ok(())
}
