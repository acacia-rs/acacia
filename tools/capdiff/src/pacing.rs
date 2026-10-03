//! `capdiff pacing <left.datagrams.tsv> <right.datagrams.tsv>`: how two clients pace RakNet
//! datagrams, from acacia-mitm's `MITM_TRACE=1` logs (one player per log). Compares the header-byte
//! mix, ACK cadence, and how bulk sends (Login and other split packets) go out between the server's
//! ACKs: a congestion window shows as flights that grow per ACK, an unpaced sender as one flight.

/// Datagrams at least this big are bulk: parts of a split packet.
const BULK_LEN: usize = 1000;
/// A pause this long ends a bulk run.
const BULK_GAP_MS: f64 = 100.0;
const RUNS_SHOWN: usize = 3;

struct Datagram {
    t: f64,
    from_game: bool,
    first_byte: u8,
    len: usize,
}

fn is_data(b: u8) -> bool {
    b & 0x80 != 0 && b & 0x60 == 0
}

fn is_ack(b: u8) -> bool {
    b & 0xc0 == 0xc0
}

/// The busiest peer's datagrams: the join, not the server-list pings the game sends from other ports.
fn load(path: &str) -> Result<Vec<Datagram>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut per_peer = std::collections::HashMap::<&str, usize>::new();
    for line in text.lines().skip(1) {
        *per_peer.entry(line.split('\t').nth(2).unwrap_or_default()).or_default() += 1;
    }
    let game = per_peer.into_iter().max_by_key(|&(_, n)| n).map(|(peer, _)| peer);
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        let [t, dir, peer, first_byte, len] = f[..] else { return Err(format!("{path}: bad line {line:?}")) };
        if Some(peer) != game {
            continue;
        }
        let parse_err = |what| format!("{path}: bad {what} in {line:?}");
        out.push(Datagram {
            t: t.parse().map_err(|_| parse_err("time"))?,
            from_game: dir == "C>S",
            first_byte: u8::from_str_radix(first_byte.trim_start_matches("0x"), 16).map_err(|_| parse_err("byte"))?,
            len: len.parse().map_err(|_| parse_err("length"))?,
        });
    }
    Ok(out)
}

pub fn run(left: &str, right: &str) -> Result<(), String> {
    let (a, b) = (load(left)?, load(right)?);
    header_mix(&a, &b);
    ack_cadence(&a, &b);
    println!("\n== bulk sends (datagrams >= {BULK_LEN} B), flights between server ACKs");
    for (name, d) in [("left", &a), ("right", &b)] {
        for (i, run) in bulk_runs(d).iter().take(RUNS_SHOWN).enumerate() {
            let (first, last) = (&d[run[0]], &d[*run.last().unwrap()]);
            println!("{name} run {i}: {} datagrams over {:.1} ms, flights {:?}", run.len(), last.t - first.t, flights(d, run));
        }
    }
    Ok(())
}

fn header_mix(a: &[Datagram], b: &[Datagram]) {
    println!("== game datagram header bytes (left / right)");
    let count = |d: &[Datagram], byte| d.iter().filter(|x| x.from_game && x.first_byte == byte).count();
    let mut bytes: Vec<u8> = a.iter().chain(b).filter(|x| x.from_game).map(|x| x.first_byte).collect();
    bytes.sort_unstable();
    bytes.dedup();
    for byte in bytes {
        println!("{byte:#04x}  {:>6} / {:<6}", count(a, byte), count(b, byte));
    }
}

fn ack_cadence(a: &[Datagram], b: &[Datagram]) {
    println!("\n== game ACK gaps, ms: median / p90 (left | right)");
    let stats = |d: &[Datagram]| {
        let times: Vec<f64> = d.iter().filter(|x| x.from_game && is_ack(x.first_byte)).map(|x| x.t).collect();
        let mut gaps: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).collect();
        gaps.sort_by(f64::total_cmp);
        let pick = |q: f64| gaps.get(((gaps.len() as f64 - 1.0) * q).round() as usize).copied().unwrap_or(f64::NAN);
        format!("{:.1} / {:.1} over {} ACKs", pick(0.5), pick(0.9), times.len())
    };
    println!("{} | {}", stats(a), stats(b));
}

/// Indices of the game's bulk data datagrams, grouped into runs split at long pauses.
fn bulk_runs(d: &[Datagram]) -> Vec<Vec<usize>> {
    let mut runs: Vec<Vec<usize>> = Vec::new();
    for (i, x) in d.iter().enumerate().filter(|(_, x)| x.from_game && is_data(x.first_byte) && x.len >= BULK_LEN) {
        match runs.last_mut() {
            Some(run) if x.t - d[*run.last().unwrap()].t <= BULK_GAP_MS => run.push(i),
            _ => runs.push(vec![i]),
        }
    }
    runs
}

/// How many of the run's datagrams went out between consecutive server ACKs.
fn flights(d: &[Datagram], run: &[usize]) -> Vec<usize> {
    let (start, end) = (run[0], *run.last().unwrap());
    let mut flights = vec![0];
    for (i, x) in d.iter().enumerate().take(end + 1).skip(start) {
        if !x.from_game && is_ack(x.first_byte) && *flights.last().unwrap() > 0 {
            flights.push(0);
        } else if run.binary_search(&i).is_ok() {
            *flights.last_mut().unwrap() += 1;
        }
    }
    flights.retain(|&n| n > 0);
    flights
}
