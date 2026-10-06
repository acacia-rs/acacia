//! What encoding costs, per packet type: every packet of the bundled BDS join is decoded, then timed being written back.
//! `cargo run --release -p acacia-testserver --example encode_cost -- [<PacketName>|all [seconds]]`
//! A packet name (or `all`) loops on those packets for a sampling profiler instead of printing the table.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::{Duration, Instant};

use acacia_proto::packets::{self, ItemRegistry};
use acacia_proto::{Packet, RawPacket, encode_packet, manual, packet_name};
use acacia_testserver::{Script, Step};
use bytes::BytesMut;

type Encode = Box<dyn Fn(&mut BytesMut)>;

macro_rules! encoder_table {
    ($($t:ident),* $(,)?) => {
        /// The packet `raw` holds, decoded and ready to be written again.
        fn encoder(raw: &RawPacket) -> Option<Encode> {
            $(if raw.id == <packets::$t as Packet>::ID {
                let packet = raw.decode::<packets::$t>().ok()?;
                return Some(Box::new(move |w| encode_packet(&packet, w)));
            })*
            None
        }
    };
}
acacia_proto::for_each_packet!(encoder_table);

const ROUNDS: usize = 200;

struct Group {
    encoders: Vec<Encode>,
    bytes: usize,
    /// Fastest of the rounds: the minimum shrugs off a busy machine.
    nanos: u128,
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let raws: Vec<RawPacket> = Script::bds_spawn()
        .steps
        .into_iter()
        .filter_map(|s| match s {
            Step::Send { packet, .. } => RawPacket::parse(packet).ok(),
            Step::Wait(_) => None,
        })
        .collect();
    // As the session does: item decoding branches on the server's shield id.
    for raw in raws.iter().filter(|r| r.is::<ItemRegistry>()) {
        if let Ok(Some(shield)) = manual::shield_item_id_in_registry(&raw.body) {
            manual::set_shield_item_id(shield);
        }
    }

    let mut groups: BTreeMap<&str, Group> = BTreeMap::new();
    let mut out = BytesMut::new();
    for raw in &raws {
        let Some(encode) = encoder(raw) else { continue };
        let before = out.len();
        encode(&mut out);
        let name = packet_name(raw.id).unwrap_or("?");
        if args.first().is_some_and(|only| only != "all" && only != name) {
            continue;
        }
        let group = groups.entry(name).or_insert(Group { encoders: Vec::new(), bytes: 0, nanos: u128::MAX });
        group.bytes += out.len() - before;
        group.encoders.push(encode);
    }
    if groups.is_empty() {
        return Err("no such packet was received".into());
    }

    if !args.is_empty() {
        let seconds = args.get(1).map_or(Ok(5.0), |s| s.parse::<f64>()).map_err(|e| e.to_string())?;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs_f64(seconds) {
            out.clear();
            groups.values().flat_map(|g| &g.encoders).for_each(|encode| encode(&mut out));
            black_box(out.len());
        }
        return Ok(());
    }

    for _ in 0..ROUNDS {
        for group in groups.values_mut() {
            out.clear();
            let start = Instant::now();
            group.encoders.iter().for_each(|encode| encode(&mut out));
            group.nanos = group.nanos.min(start.elapsed().as_nanos());
            black_box(out.len());
        }
    }
    let mut rows: Vec<_> = groups.into_iter().collect();
    rows.sort_by_key(|(_, g)| std::cmp::Reverse(g.nanos));
    let (packets, bytes, nanos) = rows.iter().fold((0, 0, 0), |t, (_, g)| (t.0 + g.encoders.len(), t.1 + g.bytes, t.2 + g.nanos));
    println!("{packets} packets, {bytes} bytes: {:.2} ms, {:.0} MB/s", nanos as f64 / 1e6, bytes as f64 * 1e3 / nanos as f64);
    println!("{:<30} {:>6} {:>9} {:>9} {:>7}", "packet", "count", "bytes", "µs", "MB/s");
    for (name, g) in rows.iter().take(14) {
        println!("{:<30} {:>6} {:>9} {:>9.1} {:>7.0}", name, g.encoders.len(), g.bytes, g.nanos as f64 / 1e3, g.bytes as f64 * 1e3 / g.nanos.max(1) as f64);
    }
    Ok(())
}
