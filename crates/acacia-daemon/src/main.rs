//! `acacia-daemon [--listen 127.0.0.1:19180] [--token SECRET] [--shards N] [--join-spacing-ms N]
//! [--join-jitter-ms N] [--max-batch-bytes N] [--profiles-folder DIR]`: serves acacia bots over WebSocket (protocol:
//! the crate docs; the host options and their defaults: acacia-host's `HostConfig`). `ACACIA_TOKEN` sets the token too. Prints
//! `listening on ws://ADDR` once ready.

use std::io::Write;
use std::process::ExitCode;
use std::str::FromStr;
use std::time::Duration;

use acacia_daemon::{Daemon, DaemonConfig};

const USAGE: &str = "usage: acacia-daemon [--listen ADDR] [--token SECRET] [--shards N] [--join-spacing-ms N] \
                     [--join-jitter-ms N] [--max-batch-bytes N] [--profiles-folder DIR]";

fn parse(args: &[String], token: Option<String>) -> Result<(String, DaemonConfig), String> {
    let mut listen = "127.0.0.1:19180".to_owned();
    let mut config = DaemonConfig { token, ..DaemonConfig::default() };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{flag} needs a value\n{USAGE}"));
        let host = &mut config.host;
        match flag.as_str() {
            "--listen" => listen = value()?,
            "--token" => config.token = Some(value()?),
            "--shards" => host.shards = number(flag, &value()?)?,
            "--join-spacing-ms" => host.join_spacing = Duration::from_millis(number(flag, &value()?)?),
            "--join-jitter-ms" => host.join_jitter = Duration::from_millis(number(flag, &value()?)?),
            "--max-batch-bytes" => host.max_batch_bytes = number(flag, &value()?)?,
            "--profiles-folder" => host.profiles_folder = Some(value()?.into()),
            "-h" | "--help" => return Err(USAGE.to_owned()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    Ok((listen, config))
}

fn number<T: FromStr<Err: std::fmt::Display>>(flag: &str, value: &str) -> Result<T, String> {
    value.parse().map_err(|e| format!("{flag}: {e}"))
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
    let (listen, config) = match parse(&args, std::env::var("ACACIA_TOKEN").ok().filter(|t| !t.is_empty())) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &str) -> Vec<String> {
        line.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn host_options_default_to_host_config_and_take_flags() {
        let (listen, config) = parse(&[], None).unwrap();
        assert_eq!(listen, "127.0.0.1:19180");
        let default = acacia_host::HostConfig::default();
        assert_eq!((config.host.join_spacing, config.host.join_jitter), (default.join_spacing, default.join_jitter));
        assert_eq!(config.host.max_batch_bytes, 128 << 20);

        let (_, config) = parse(&args("--join-spacing-ms 100 --join-jitter-ms 0 --max-batch-bytes 1024 --shards 2"), None).unwrap();
        assert_eq!(config.host.join_spacing, Duration::from_millis(100));
        assert_eq!(config.host.join_jitter, Duration::ZERO);
        assert_eq!((config.host.max_batch_bytes, config.host.shards), (1024, 2));
        let (_, config) = parse(&args("--profiles-folder /srv/tokens"), None).unwrap();
        assert_eq!(config.host.profiles_folder.as_deref(), Some(std::path::Path::new("/srv/tokens")));
        assert!(parse(&args("--join-spacing-ms soon"), None).unwrap_err().starts_with("--join-spacing-ms: "));
    }
}
