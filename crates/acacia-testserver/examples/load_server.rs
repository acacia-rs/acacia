//! A local server for profiling a swarm: every client gets the bundled BDS join, then the steady
//! traffic of a recorded session (a directory written by acacia-client's `capture` example) on a loop.
//! `cargo run --release -p acacia-testserver --example load_server -- <port> <capture dir> [loops] [capture seconds]`
//! `capture seconds` is how long the recording ran, so the replay keeps its packet rate.

use std::path::Path;
use std::time::Duration;

use acacia_proto::packets::{AddEntity, AddPlayer, Animate, MobEquipment, MoveEntity, PlayerList, RemoveEntity, SetEntityData, UpdateAttributes};
use acacia_proto::{Packet, RawPacket};
use acacia_testserver::{Script, Step};
use bytes::Bytes;

/// What a hub sends a standing player: other entities' state. Nothing that moves or reconfigures the client.
const STEADY: &[u32] =
    &[MoveEntity::ID, SetEntityData::ID, UpdateAttributes::ID, MobEquipment::ID, AddPlayer::ID, AddEntity::ID, RemoveEntity::ID, PlayerList::ID, Animate::ID];
const TICK: Duration = Duration::from_millis(50);

/// The capture's steady packets (with headers), in order.
fn steady_packets(dir: &Path) -> std::io::Result<Vec<Bytes>> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| Some(e.ok()?.path())).collect();
    paths.sort();
    let mut packets = Vec::new();
    for path in paths {
        let packet = Bytes::from(std::fs::read(path)?);
        if RawPacket::parse(packet.clone()).is_ok_and(|p| STEADY.contains(&p.id)) {
            packets.push(packet);
        }
    }
    Ok(packets)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [port, dir, rest @ ..] = args.as_slice() else {
        return Err("usage: load_server <port> <capture dir> [loops] [capture seconds]".into());
    };
    let loops: usize = rest.first().map_or(Ok(3), |s| s.parse())?;
    let seconds: f64 = rest.get(1).map_or(Ok(90.0), |s| s.parse())?;

    let packets = steady_packets(Path::new(dir))?;
    let ticks = (seconds / TICK.as_secs_f64()) as usize;
    let per_tick = packets.len().div_ceil(ticks.max(1)).max(1);
    let mut script = Script::bds_spawn();
    for _ in 0..loops {
        for (i, packet) in packets.iter().enumerate() {
            let delay = if i % per_tick == 0 { TICK } else { Duration::ZERO };
            script.steps.push(Step::Send { delay, packet: packet.clone() });
        }
    }
    let socket = tokio::net::UdpSocket::bind(("127.0.0.1", port.parse::<u16>()?)).await?;
    println!("{} steady packets, {per_tick} per tick, {loops} loops, on {}", packets.len(), socket.local_addr()?);
    acacia_testserver::load::serve(socket, script).await;
    Ok(())
}
