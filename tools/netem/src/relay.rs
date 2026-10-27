//! The proxy: datagrams from clients go to the server from a socket of each client's own, and the
//! server's answers come back from the listening socket, each through its direction's [`Link`].
//! One thread sends what is due, in order of arrival time.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Instant;

use crate::capture::Capture;
use crate::link::{Fate, Impairment, Link};

const MAX_DATAGRAM: usize = 65_536;

/// What one direction did, for the stats line.
#[derive(Default)]
pub struct Counters {
    pub received: AtomicU64,
    pub lost: AtomicU64,
    pub overflowed: AtomicU64,
    pub sent: AtomicU64,
    pub bytes: AtomicU64,
    pub max_backlog: AtomicU64,
}

struct Due {
    at: Instant,
    order: u64,
    socket: Arc<UdpSocket>,
    /// `None` for an upstream socket, connected to the server.
    to: Option<SocketAddr>,
    data: Vec<u8>,
}

impl PartialEq for Due {
    fn eq(&self, other: &Self) -> bool {
        (self.at, self.order) == (other.at, other.order)
    }
}

impl Eq for Due {}

impl PartialOrd for Due {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Earliest first in a `BinaryHeap`.
impl Ord for Due {
    fn cmp(&self, other: &Self) -> Ordering {
        (other.at, other.order).cmp(&(self.at, self.order))
    }
}

#[derive(Default)]
struct Queue {
    due: Mutex<(BinaryHeap<Due>, u64)>,
    wake: Condvar,
}

impl Queue {
    fn push(&self, at: Instant, socket: Arc<UdpSocket>, to: Option<SocketAddr>, data: Vec<u8>) {
        let mut due = self.due.lock().expect("queue lock");
        let order = due.1;
        due.1 += 1;
        due.0.push(Due { at, order, socket, to, data });
        self.wake.notify_one();
    }

    fn send_forever(&self, counters: [&Counters; 2]) {
        let mut due = self.due.lock().expect("queue lock");
        loop {
            let now = Instant::now();
            match due.0.peek().map(|next| next.at) {
                Some(at) if at <= now => {
                    let next = due.0.pop().expect("peeked");
                    let sent = match next.to {
                        Some(to) => next.socket.send_to(&next.data, to),
                        None => next.socket.send(&next.data),
                    };
                    if sent.is_ok() {
                        let counters = counters[usize::from(next.to.is_some())];
                        counters.sent.fetch_add(1, Relaxed);
                        counters.bytes.fetch_add(next.data.len() as u64, Relaxed);
                    }
                }
                Some(at) => due = self.wake.wait_timeout(due, at - now).expect("queue lock").0,
                None => due = self.wake.wait(due).expect("queue lock"),
            }
        }
    }
}

pub struct Proxy {
    pub up: Counters,
    pub down: Counters,
    pub capture: Option<Capture>,
    queue: Queue,
}

impl Proxy {
    /// Passes a datagram going `up` (or down) through `link`, queueing it to be sent where it
    /// arrives.
    fn pass(&self, up: bool, link: &Mutex<Link>, socket: &Arc<UdpSocket>, to: Option<SocketAddr>, data: &[u8]) {
        let counters = if up { &self.up } else { &self.down };
        counters.received.fetch_add(1, Relaxed);
        let now = Instant::now();
        let mut link = link.lock().expect("link lock");
        let fate = link.admit(now, data.len());
        counters.max_backlog.fetch_max(link.backlog(now) as u64, Relaxed);
        drop(link);
        if let Some(capture) = &self.capture {
            capture.record(up, fate, data);
        }
        match fate {
            Fate::Lost => drop(counters.lost.fetch_add(1, Relaxed)),
            Fate::Overflow => drop(counters.overflowed.fetch_add(1, Relaxed)),
            Fate::Arrives(at) => self.queue.push(at, socket.clone(), to, data.to_vec()),
        }
    }
}

/// Starts the proxy on `listen` for `server`; its threads run until the process ends.
pub fn start(listen: SocketAddr, server: SocketAddr, up: Impairment, down: Impairment, seed: u64, capture: Option<Capture>) -> std::io::Result<Arc<Proxy>> {
    let socket = Arc::new(UdpSocket::bind(listen)?);
    let proxy = Arc::new(Proxy { up: Counters::default(), down: Counters::default(), capture, queue: Queue::default() });
    let sender = proxy.clone();
    thread::spawn(move || sender.queue.send_forever([&sender.up, &sender.down]));
    let relay = proxy.clone();
    thread::spawn(move || {
        let mut flows: HashMap<SocketAddr, (Arc<UdpSocket>, Arc<Mutex<Link>>)> = HashMap::new();
        let mut buffer = vec![0; MAX_DATAGRAM];
        loop {
            let (size, client) = match socket.recv_from(&mut buffer) {
                Ok(got) => got,
                Err(error) if resets(&error) => continue,
                Err(_) => break,
            };
            if !flows.contains_key(&client) {
                let Ok(upstream) = open_upstream(server) else { continue };
                let now = Instant::now();
                let seed = seed.wrapping_add(flows.len() as u64 * 2);
                let downlink = Mutex::new(Link::new(down, now, seed ^ 0x9E37_79B9_7F4A_7C15));
                spawn_downstream(relay.clone(), upstream.clone(), socket.clone(), client, downlink);
                flows.insert(client, (upstream, Arc::new(Mutex::new(Link::new(up, now, seed)))));
            }
            let (upstream, uplink) = &flows[&client];
            relay.pass(true, uplink, upstream, None, &buffer[..size]);
        }
    });
    Ok(proxy)
}

fn open_upstream(server: SocketAddr) -> std::io::Result<Arc<UdpSocket>> {
    let any: SocketAddr = if server.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }.parse().expect("an address");
    let upstream = UdpSocket::bind(any)?;
    upstream.connect(server)?;
    Ok(Arc::new(upstream))
}

/// The server's answers to `client`, back out of the listening socket.
fn spawn_downstream(proxy: Arc<Proxy>, upstream: Arc<UdpSocket>, socket: Arc<UdpSocket>, client: SocketAddr, link: Mutex<Link>) {
    thread::spawn(move || {
        let mut buffer = vec![0; MAX_DATAGRAM];
        loop {
            match upstream.recv(&mut buffer) {
                Ok(size) => proxy.pass(false, &link, &socket, Some(client), &buffer[..size]),
                Err(error) if resets(&error) => continue,
                Err(_) => break,
            }
        }
    });
}

/// Windows reports an ICMP "port unreachable" for an earlier send as a reset on the next
/// receive; the socket goes on working.
fn resets(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::ConnectionReset
}
