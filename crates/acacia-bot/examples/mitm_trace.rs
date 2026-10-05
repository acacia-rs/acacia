//! Converts an acacia-mitm capture of the vanilla client into movement traces for the `replay` example,
//! one per session (login): server packets become trace packets, `PlayerAuthInput`s become inputs.
//! `cargo run -p acacia-bot --example mitm_trace -- <capture.jsonl> <out-prefix>`
//! then `replay <out-prefix>-<n>.btrc --resync` (a real client: resync after each divergence).

use std::collections::HashMap;

use acacia_bot::trace::{Event, Recorder};
use acacia_client::proto::packets::PlayerAuthInput;
use acacia_client::proto::{Packet, RawPacket};

/// One session's trace, numbered by its login's place in the capture.
struct Trace {
    n: u32,
    rec: Recorder,
    packets: u32,
    inputs: u32,
}

impl Trace {
    fn report(&self) {
        println!("session {}: {} packets, {} inputs", self.n, self.packets, self.inputs);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(capture), Some(prefix)) = (args.next(), args.next()) else {
        return Err("usage: mitm_trace <capture.jsonl> <out-prefix>".into());
    };
    let text = std::fs::read_to_string(capture)?;
    let mut logins = 0;
    let mut traces: HashMap<&str, Trace> = HashMap::new();
    for line in text.lines() {
        // Keys are sorted, so the top-level "session" and "raw" are the last ones on the line.
        // Captures from before "session" hold one connection at a time.
        let key = line.rfind(r#""session":"#).and_then(|at| field(&line[at..], r#""session":"#, ',')).unwrap_or("");
        if line.contains(r#""event":"login""#) {
            logins += 1;
            let path = format!("{prefix}-{logins}.btrc");
            let trace = Trace { n: logins, rec: Recorder::create(path.as_ref())?, packets: 0, inputs: 0 };
            if let Some(done) = traces.insert(key, trace) {
                done.report();
            }
            continue;
        }
        let (Some(trace), Some(id), Some(raw)) = (traces.get_mut(key), field(line, r#""id":"#, ','), line.rfind(r#""raw":""#)) else { continue };
        let raw = &line[raw + 7..];
        let body = hex(&raw[..raw.find('"').unwrap_or(raw.len())]);
        let id: u32 = id.parse()?;
        // "dir" is "S>C" (escaped as S>C) or "C>S".
        if line.starts_with(r#"{"dir":"S"#) {
            trace.rec.write(&Event::Packet(RawPacket { id, sender_subclient: 0, target_subclient: 0, body: body.into() }));
            trace.packets += 1;
        } else if id == PlayerAuthInput::ID && line.starts_with(r#"{"dir":"C"#) {
            trace.rec.write(&Event::Input(body.into()));
            trace.inputs += 1;
        }
    }
    let mut open: Vec<_> = traces.into_values().collect();
    open.sort_by_key(|t| t.n);
    open.iter().for_each(Trace::report);
    Ok(())
}

/// The text after `key` up to `end`.
fn field<'a>(line: &'a str, key: &str, end: char) -> Option<&'a str> {
    let rest = &line[line.find(key)? + key.len()..];
    Some(&rest[..rest.find(end)?])
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap_or(0)).collect()
}
