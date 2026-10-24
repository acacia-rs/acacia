//! A UDP relay that delays, jitters and drops datagrams, for trying a server and its clients
//! over a bad link: a client connects to the proxy instead of the server. Each direction is delayed
//! by `--delay` plus up to `--jitter` milliseconds (so datagrams can arrive out of order) and loses
//! `--loss` percent of its datagrams; `--up-loss` and `--down-loss` set one direction apart.
//! Every ten seconds it prints how many datagrams went each way and how many it dropped.
//!
//! `cargo run -p acacia-raknet --example lossy_proxy -- 0.0.0.0:19200 127.0.0.1:19132 --delay 80 --jitter 40 --loss 2`

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::net::UdpSocket;

/// What happens to datagrams going one way.
#[derive(Clone, Copy, Debug)]
struct Link {
    delay: Duration,
    jitter: Duration,
    /// Per million datagrams.
    loss: u64,
}

#[derive(Default)]
struct Counts {
    up: AtomicU64,
    up_dropped: AtomicU64,
    down: AtomicU64,
    down_dropped: AtomicU64,
}

/// xorshift64*: repeatable from `--seed`, and no dependency for an example.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// When to send a datagram over `link`, or `None` to drop it.
    fn fate(&mut self, link: Link) -> Option<Duration> {
        if self.next() % 1_000_000 < link.loss {
            return None;
        }
        let jitter = link.jitter.as_micros() as u64;
        Some(link.delay + Duration::from_micros(if jitter == 0 { 0 } else { self.next() % (jitter + 1) }))
    }
}

/// Sends `datagram` on `socket` to `to` (or its connected peer) after `after`.
fn send_later(socket: Arc<UdpSocket>, datagram: Vec<u8>, to: Option<SocketAddr>, after: Duration) {
    tokio::spawn(async move {
        tokio::time::sleep(after).await;
        let sent = match to {
            Some(to) => socket.send_to(&datagram, to).await,
            None => socket.send(&datagram).await,
        };
        if let Err(error) = sent {
            eprintln!("send failed: {error}");
        }
    });
}

fn flag(args: &[String], name: &str) -> Option<f64> {
    args.iter().position(|arg| arg == name).and_then(|at| args.get(at + 1)).map(|value| value.parse().unwrap_or_else(|_| panic!("{name} takes a number")))
}

fn link(args: &[String], loss_flag: &str) -> Link {
    let ms = |name: &str| Duration::from_micros((flag(args, name).unwrap_or(0.0) * 1000.0) as u64);
    let loss = flag(args, loss_flag).or_else(|| flag(args, "--loss")).unwrap_or(0.0);
    Link { delay: ms("--delay"), jitter: ms("--jitter"), loss: (loss * 10_000.0) as u64 }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [listen, server] = [0, 1].map(|at| args.get(at).and_then(|addr| addr.parse::<SocketAddr>().ok()).expect("usage: lossy_proxy <listen addr> <server addr> [--delay ms] [--jitter ms] [--loss %] [--up-loss %] [--down-loss %] [--seed n]"));
    let (up, down) = (link(&args, "--up-loss"), link(&args, "--down-loss"));
    let dice = Arc::new(Mutex::new(Dice(flag(&args, "--seed").unwrap_or(1.0) as u64 | 1)));
    let counts = Arc::new(Counts::default());
    let front = Arc::new(UdpSocket::bind(listen).await?);
    println!("relaying {listen} -> {server}; up {up:?}, down {down:?}");

    let report = counts.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let get = |count: &AtomicU64| count.load(Ordering::Relaxed);
            println!("up {} (dropped {}), down {} (dropped {})", get(&report.up), get(&report.up_dropped), get(&report.down), get(&report.down_dropped));
        }
    });

    // One socket towards the server per client, so the server tells clients apart as it would.
    let mut clients: HashMap<SocketAddr, Arc<UdpSocket>> = HashMap::new();
    let mut buf = vec![0u8; 65536];
    loop {
        let (len, from) = front.recv_from(&mut buf).await?;
        let back = match clients.get(&from) {
            Some(back) => back.clone(),
            None => {
                let back = Arc::new(UdpSocket::bind(if server.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }).await?);
                back.connect(server).await?;
                println!("client {from} on {}", back.local_addr()?);
                let (reply, front, dice, counts) = (back.clone(), front.clone(), dice.clone(), counts.clone());
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 65536];
                    while let Ok(len) = reply.recv(&mut buf).await {
                        let fate = dice.lock().unwrap().fate(down);
                        let Some(after) = fate else {
                            counts.down_dropped.fetch_add(1, Ordering::Relaxed);
                            continue;
                        };
                        counts.down.fetch_add(1, Ordering::Relaxed);
                        send_later(front.clone(), buf[..len].to_vec(), Some(from), after);
                    }
                });
                clients.insert(from, back.clone());
                back
            }
        };
        let fate = dice.lock().unwrap().fate(up);
        let Some(after) = fate else {
            counts.up_dropped.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        counts.up.fetch_add(1, Ordering::Relaxed);
        send_later(back, buf[..len].to_vec(), None, after);
    }
}
