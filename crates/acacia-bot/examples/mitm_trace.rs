//! Converts an acacia-mitm capture of the vanilla client into movement traces for the `replay` example,
//! one per session (login): server packets become trace packets, `PlayerAuthInput`s become inputs.
//! `cargo run -p acacia-bot --example mitm_trace -- <capture.jsonl> <out-prefix>`
//! then `replay <out-prefix>-<n>.btrc --resync` (a real client: resync after each divergence).

use acacia_bot::trace::{Event, Recorder};
use acacia_client::proto::packets::PlayerAuthInput;
use acacia_client::proto::{Packet, RawPacket};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(capture), Some(prefix)) = (args.next(), args.next()) else {
        return Err("usage: mitm_trace <capture.jsonl> <out-prefix>".into());
    };
    let text = std::fs::read_to_string(capture)?;
    let mut session = 0;
    let mut out: Option<Recorder> = None;
    let (mut packets, mut inputs) = (0, 0);
    for line in text.lines() {
        // Go marshals map keys sorted, so the top-level "raw" is the last one on the line.
        if line.contains(r#""event":"login""#) {
            if out.is_some() {
                println!("session {session}: {packets} packets, {inputs} inputs");
            }
            session += 1;
            let path = format!("{prefix}-{session}.btrc");
            out = Some(Recorder::create(path.as_ref())?);
            (packets, inputs) = (0, 0);
            continue;
        }
        let (Some(rec), Some(id), Some(raw)) = (out.as_mut(), field(line, r#""id":"#, ','), line.rfind(r#""raw":""#)) else { continue };
        let raw = &line[raw + 7..];
        let body = hex(&raw[..raw.find('"').unwrap_or(raw.len())]);
        let id: u32 = id.parse()?;
        // "dir" is "S>C" (escaped as S>C) or "C>S".
        if line.starts_with(r#"{"dir":"S"#) {
            rec.write(&Event::Packet(RawPacket { id, sender_subclient: 0, target_subclient: 0, body: body.into() }));
            packets += 1;
        } else if id == PlayerAuthInput::ID {
            rec.write(&Event::Input(body.into()));
            inputs += 1;
        }
    }
    if out.is_some() {
        println!("session {session}: {packets} packets, {inputs} inputs");
    }
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
