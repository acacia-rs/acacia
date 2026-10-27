//! A UDP proxy that makes a link worse, for testing a server or client over one (README.md).

mod capture;
mod link;
mod relay;

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::atomic::Ordering::Relaxed;
use std::thread;
use std::time::{Duration, Instant};

use link::Impairment;
use relay::{Counters, Proxy};

const USAGE: &str = "usage: netem --listen ADDR --to ADDR [--delay 150ms] [--jitter 20ms] [--loss 2] [--rate 1mbit] [--queue 256k]
             [--up-<option> V | --down-<option> V] [--seed N] [--stats 1] [--for SECONDS] [--capture FILE]
  up is client to server, down server to client; an option without a direction sets both.
  --loss is a percentage, --rate bits a second (kbit, mbit), --queue bytes (k, m) waiting for the rate.
  --stats prints a line every so many seconds (0: only the last); --for stops after so many.
  --capture writes every datagram to FILE (capture.rs) for raknet_trace.py.";

struct Options {
    listen: SocketAddr,
    to: SocketAddr,
    up: Impairment,
    down: Impairment,
    seed: u64,
    stats: f64,
    run_for: Option<f64>,
    capture: Option<String>,
}

fn duration(text: &str) -> Option<Duration> {
    let (number, scale) = match text.strip_suffix("ms") {
        Some(number) => (number, 1e-3),
        None => text.strip_suffix('s').map_or((text, 1e-3), |number| (number, 1.0)),
    };
    number.parse::<f64>().ok().filter(|n| *n >= 0.0).map(|n| Duration::from_secs_f64(n * scale))
}

/// A number with an optional decimal suffix from `scales`, e.g. `1.5mbit`.
fn scaled(text: &str, scales: &[(&str, f64)]) -> Option<u64> {
    let lower = text.to_ascii_lowercase();
    let (number, scale) = scales.iter().find_map(|(suffix, scale)| lower.strip_suffix(suffix).map(|number| (number.to_owned(), *scale))).unwrap_or((lower, 1.0));
    number.parse::<f64>().ok().filter(|n| *n >= 0.0).map(|n| (n * scale) as u64)
}

fn set(impairment: &mut Impairment, option: &str, value: &str) -> Option<()> {
    match option {
        "delay" => impairment.delay = duration(value)?,
        "jitter" => impairment.jitter = duration(value)?,
        "loss" => impairment.loss = value.parse::<f64>().ok().filter(|p| (0.0..=100.0).contains(p))? / 100.0,
        "rate" => impairment.rate = Some(scaled(value, &[("gbit", 1e9), ("mbit", 1e6), ("kbit", 1e3), ("bit", 1.0)]).filter(|rate| *rate > 0)?),
        "queue" => impairment.queue = scaled(value, &[("m", 1024.0 * 1024.0), ("k", 1024.0)])? as usize,
        _ => return None,
    }
    Some(())
}

fn parse(args: &[String]) -> Result<Options, String> {
    let (mut listen, mut to, mut up, mut down) = (None, None, Impairment::default(), Impairment::default());
    let (mut seed, mut stats, mut run_for, mut capture) = (1, 1.0, None, None);
    for pair in args.chunks(2) {
        let [flag, value] = pair else { return Err(format!("{} wants a value", pair[0])) };
        let wrong = || format!("bad {flag}: {value}");
        let option = flag.strip_prefix("--").ok_or_else(wrong)?;
        match option {
            "listen" => listen = Some(value.parse().map_err(|_| wrong())?),
            "to" => to = Some(value.parse().map_err(|_| wrong())?),
            "seed" => seed = value.parse().map_err(|_| wrong())?,
            "stats" => stats = value.parse().map_err(|_| wrong())?,
            "for" => run_for = Some(value.parse().map_err(|_| wrong())?),
            "capture" => capture = Some(value.clone()),
            _ => {
                let set_in = |impairment: &mut Impairment, option: &str| set(impairment, option, value).ok_or_else(wrong);
                match (option.strip_prefix("up-"), option.strip_prefix("down-")) {
                    (Some(option), _) => set_in(&mut up, option)?,
                    (_, Some(option)) => set_in(&mut down, option)?,
                    _ => {
                        set_in(&mut up, option)?;
                        set_in(&mut down, option)?;
                    }
                }
            }
        }
    }
    let (Some(listen), Some(to)) = (listen, to) else { return Err("--listen and --to are needed".into()) };
    Ok(Options { listen, to, up, down, seed, stats, run_for, capture })
}

fn side(name: &str, counters: &Counters) -> String {
    let get = |counter: &std::sync::atomic::AtomicU64| counter.load(Relaxed);
    format!(
        "{name} in {} lost {} full {} out {} kB {} max-queue {}",
        get(&counters.received),
        get(&counters.lost),
        get(&counters.overflowed),
        get(&counters.sent),
        get(&counters.bytes) / 1024,
        get(&counters.max_backlog)
    )
}

fn line(proxy: &Proxy, label: &str) -> String {
    format!("{label} | {} | {}", side("up", &proxy.up), side("down", &proxy.down))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse(&args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let started = Instant::now();
    let capture = match options.capture.as_deref().map(|path| capture::Capture::create(path, started)).transpose() {
        Ok(capture) => capture,
        Err(error) => {
            eprintln!("netem: --capture: {error}");
            return ExitCode::FAILURE;
        }
    };
    let proxy = match relay::start(options.listen, options.to, options.up, options.down, options.seed, capture) {
        Ok(proxy) => proxy,
        Err(error) => {
            eprintln!("netem: {}: {error}", options.listen);
            return ExitCode::FAILURE;
        }
    };
    println!("netem {} -> {} | up {:?} | down {:?}", options.listen, options.to, options.up, options.down);
    let every = Duration::from_secs_f64(if options.stats > 0.0 { options.stats } else { 1.0 });
    loop {
        thread::sleep(every);
        if let Some(capture) = &proxy.capture {
            capture.flush();
        }
        let elapsed = started.elapsed().as_secs_f64();
        if options.run_for.is_some_and(|stop| elapsed >= stop) {
            println!("{}", line(&proxy, "total"));
            return ExitCode::SUCCESS;
        }
        if options.stats > 0.0 {
            println!("{}", line(&proxy, &format!("{elapsed:.0}s")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn options_set_both_directions_or_one() {
        let options = parse(&args("--listen 127.0.0.1:19140 --to 127.0.0.1:19132 --delay 75ms --down-rate 1.5mbit --loss 2 --up-jitter 1s")).unwrap();
        assert_eq!((options.up.delay, options.down.delay), (Duration::from_millis(75), Duration::from_millis(75)));
        assert_eq!((options.up.rate, options.down.rate), (None, Some(1_500_000)));
        assert_eq!((options.up.loss, options.up.jitter, options.down.jitter), (0.02, Duration::from_secs(1), Duration::ZERO));
        assert!(parse(&args("--listen 127.0.0.1:1 --to 127.0.0.1:2 --loss 150")).is_err());
        assert!(parse(&args("--listen 127.0.0.1:1")).is_err());
    }
}
