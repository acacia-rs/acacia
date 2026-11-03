//! `acacia-daemon [--listen 127.0.0.1:19180] [--token SECRET] [--shards N]`: serves acacia bots over WebSocket
//! (protocol: the crate docs). `ACACIA_TOKEN` sets the token too. Prints `listening on ws://ADDR` once ready.

use std::io::Write;
use std::process::ExitCode;

use acacia_daemon::{Daemon, DaemonConfig};

const USAGE: &str = "usage: acacia-daemon [--listen ADDR] [--token SECRET] [--shards N]";

fn parse(args: &[String]) -> Result<(String, DaemonConfig), String> {
    let mut listen = "127.0.0.1:19180".to_owned();
    let mut config = DaemonConfig { token: std::env::var("ACACIA_TOKEN").ok().filter(|t| !t.is_empty()), ..DaemonConfig::default() };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{flag} needs a value\n{USAGE}"));
        match flag.as_str() {
            "--listen" => listen = value()?,
            "--token" => config.token = Some(value()?),
            "--shards" => config.host.shards = value()?.parse().map_err(|e| format!("--shards: {e}"))?,
            "-h" | "--help" => return Err(USAGE.to_owned()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    Ok((listen, config))
}

async fn stopped() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("a SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (listen, config) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let daemon = match Daemon::bind(&listen, config).await {
        Ok(daemon) => daemon,
        Err(e) => {
            eprintln!("acacia-daemon: cannot listen on {listen}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("listening on ws://{}", daemon.local_addr());
    let _ = std::io::stdout().flush();
    stopped().await;
    daemon.shutdown().await;
    ExitCode::SUCCESS
}
