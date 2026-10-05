//! Decodes every packet of acacia-mitm captures with acacia-proto and encodes it again: the codec
//! check the proxy only does for small bodies while recording (acacia-mitm record.rs). Lists each
//! packet name that failed to decode or came back as different bytes, with its first failure, and
//! exits 1 if there was any.
//!
//! `cargo run -p acacia-testserver --example capcheck -- <capture.jsonl> [more captures]`
use std::collections::BTreeMap;

use acacia_proto::Packet as _;
use acacia_proto::packets;
use acacia_testserver::capture::{self, Packet};
use bytes::BytesMut;

#[derive(Default)]
struct Tally {
    seen: u32,
    undecoded: u32,
    differs: u32,
    first: Option<String>,
}

enum Check {
    Same,
    Undecoded(String),
    Differs(String),
    /// Not an id acacia-proto has.
    Unknown,
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: capcheck <capture.jsonl> [more captures]");
        std::process::exit(2);
    }
    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    for path in &paths {
        let sessions = capture::read(path).unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(2);
        });
        for p in sessions.iter().flat_map(|s| &s.packets) {
            let tally = tallies.entry(p.name.clone()).or_default();
            tally.seen += 1;
            let failure = match check(p) {
                Check::Same => continue,
                Check::Undecoded(e) => {
                    tally.undecoded += 1;
                    format!("decode: {e}")
                }
                Check::Differs(e) => {
                    tally.differs += 1;
                    e
                }
                Check::Unknown => {
                    tally.undecoded += 1;
                    format!("unknown id {}", p.id)
                }
            };
            tally.first.get_or_insert_with(|| format!("{failure} ({path}, {:.0} ms {})", p.t, if p.from_client { "C>S" } else { "S>C" }));
        }
    }
    let (mut seen, mut undecoded, mut differs) = (0, 0, 0);
    for (name, t) in &tallies {
        (seen, undecoded, differs) = (seen + t.seen, undecoded + t.undecoded, differs + t.differs);
        if let Some(first) = &t.first {
            println!("{name}: {} of {} undecoded, {} differ; first: {first}", t.undecoded, t.seen, t.differs);
        }
    }
    println!("{seen} packets of {} kinds: {undecoded} undecoded, {differs} encoded back differently", tallies.len());
    if undecoded + differs > 0 {
        std::process::exit(1);
    }
}

macro_rules! checker {
    ($($t:ident),* $(,)?) => {
        fn check(p: &Packet) -> Check {
            $(if p.id == <packets::$t>::ID {
                return match <packets::$t>::decode(&mut &p.body[..]) {
                    Ok(v) => {
                        let mut back = BytesMut::with_capacity(p.body.len());
                        v.encode(&mut back);
                        match back[..] == p.body[..] {
                            true => Check::Same,
                            false => Check::Differs(format!("{} bytes encoded back as {}, same up to byte {}", p.body.len(), back.len(), back.iter().zip(&p.body).take_while(|(a, b)| a == b).count())),
                        }
                    }
                    Err(e) => Check::Undecoded(e.to_string()),
                };
            })*
            Check::Unknown
        }
    };
}
acacia_proto::for_each_packet!(checker);
