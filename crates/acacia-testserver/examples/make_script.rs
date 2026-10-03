//! Turns an acacia-mitm capture into a fake-server script (fixtures/*.script).
//! `cargo run -p acacia-testserver --example make_script -- <capture.jsonl[#session]> <out.script> [seconds]`
//! Record a bot, not the real game: the script ships in the repo and replays the server's PlayerList
//! and skins of whoever joined.
use std::time::Duration;

use acacia_testserver::{capture, Script, Step};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(input), Some(out)) = (args.first(), args.get(1)) else {
        return Err("usage: make_script <capture.jsonl[#n]> <out.script> [seconds after login, default 30]".into());
    };
    let seconds: f64 = args.get(2).map_or(Ok(30.0), |s| s.parse())?;
    let script = Script::from_session(&capture::session(input)?, Duration::from_secs_f64(seconds));
    let sends = script.steps.iter().filter(|s| matches!(s, Step::Send { .. })).count();
    let data = script.encode();
    std::fs::write(out, &data)?;
    println!("{sends} sends, {} waits, {} blobs, {} KB", script.steps.len() - sends, script.blobs.len(), data.len() / 1024);
    Ok(())
}
