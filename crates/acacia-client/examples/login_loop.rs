//! Logs in N times with a fresh connection and key each time, and counts the joins the server
//! closed during login (BDS drops a Login it cannot read without a Disconnect).
//! `cargo run --release -p acacia-client --example login_loop -- 127.0.0.1:19140 300 many 4 Loop`
//! - `same` joins as one name every time (one at a time); `many` gives every attempt its own,
//!   which also walks the skin pool (acacia-auth picks skin and device by name).
use std::time::Duration;

use acacia_client::{Client, ConnectError, DisconnectReason};

#[derive(Clone, Copy, PartialEq)]
enum Outcome {
    Spawned,
    Closed,
    Other,
}

async fn attempt(server: String, name: String) -> Outcome {
    match Client::builder(server).offline(&name).blob_cache_dir(".blobs").pack_cache_dir(".packs").connect().await {
        Ok(client) => {
            client.close();
            Outcome::Spawned
        }
        // Matched by name: the transport's reason type is not re-exported.
        Err(ConnectError::Disconnected(DisconnectReason::Transport(reason))) if format!("{reason:?}") == "ServerClosed" => {
            println!("{name}: closed by the server during login");
            Outcome::Closed
        }
        Err(e) => {
            println!("{name}: {e}");
            Outcome::Other
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let attempts: usize = args.next().map_or(200, |s| s.parse().expect("attempts"));
    let same = args.next().is_some_and(|s| s == "same");
    let parallel: usize = args.next().map_or(4, |s| s.parse().expect("parallel joins"));
    let parallel = if same { 1 } else { parallel.max(1) };
    let prefix = args.next().unwrap_or_else(|| "Loop".into());

    let mut outcomes = Vec::with_capacity(attempts);
    for first in (0..attempts).step_by(parallel) {
        let tasks: Vec<_> = (first..attempts.min(first + parallel))
            .map(|i| tokio::spawn(attempt(server.clone(), if same { prefix.clone() } else { format!("{prefix}{i}") })))
            .collect();
        for task in tasks {
            outcomes.push(task.await.expect("task panicked"));
        }
        // BDS refuses a name that is still leaving.
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let count = |o| outcomes.iter().filter(|&&x| x == o).count();
    println!("{attempts} attempts: {} spawned, {} closed during login, {} other", count(Outcome::Spawned), count(Outcome::Closed), count(Outcome::Other));
}
