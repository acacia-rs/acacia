//! The report sections. Left = `a`, right = `b`; times are ms since each side's Login.

use std::collections::{BTreeMap, HashSet};

use acacia_proto::{packets, Packet as _};

use acacia_testserver::capture::{Packet, Session};
use crate::diff;

/// Client packets sent this often are compared by cadence, not by individual reactions or fields.
const STREAM_MIN: usize = 20;
/// A one-off packet: compared field by field.
const ONE_OFF_MAX: usize = 3;

fn section(title: &str) {
    println!("\n== {title}");
}

fn ms(t: Option<f64>) -> String {
    t.map_or("-".into(), |t| format!("{t:.0}"))
}

pub fn summary(a: &Session, b: &Session) {
    for (side, s) in [("left ", a), ("right", b)] {
        let end = s.packets.last().map_or(0.0, |p| p.t);
        println!("{side}: {:.1} s, spawned at {} ms, {} client packets", end / 1000.0, ms(s.spawned), s.client().count());
    }
}

fn counts(s: &Session) -> BTreeMap<&str, (usize, f64)> {
    let mut out = BTreeMap::new();
    for p in s.client() {
        out.entry(p.name.as_str()).or_insert((0, p.t)).0 += 1;
    }
    out
}

pub fn inventory(a: &Session, b: &Session) {
    section("Client packets (count, first sent ms); ✗ = only one side sends it");
    let (ca, cb) = (counts(a), counts(b));
    let mut names: Vec<&str> = ca.keys().chain(cb.keys()).copied().collect::<HashSet<_>>().into_iter().collect();
    names.sort_by(|x, y| first(&ca, &cb, x).total_cmp(&first(&ca, &cb, y)));
    for name in names {
        let (l, r) = (ca.get(name), cb.get(name));
        let mark = if l.is_none() || r.is_none() { "✗" } else { " " };
        let cell = |c: Option<&(usize, f64)>| c.map_or(format!("{:>14}", "-"), |(n, t)| format!("{n:>6} @{t:>7.0}"));
        println!("{mark} {name:<36} {}   {}", cell(l), cell(r));
    }
}

fn first(ca: &BTreeMap<&str, (usize, f64)>, cb: &BTreeMap<&str, (usize, f64)>, name: &str) -> f64 {
    ca.get(name).or(cb.get(name)).map_or(f64::MAX, |c| c.1)
}

/// Client packet names up to `window` ms after spawn, with repeats collapsed; streams only at their start.
fn sequence(s: &Session, window: f64, streams: &HashSet<&str>) -> Vec<String> {
    let until = s.spawned.unwrap_or(0.0) + window;
    let mut started = HashSet::new();
    let mut out: Vec<(String, usize)> = Vec::new();
    for p in s.client().filter(|p| p.t <= until) {
        if streams.contains(p.name.as_str()) && !started.insert(p.name.as_str()) {
            continue;
        }
        match out.last_mut() {
            Some((name, n)) if *name == p.name => *n += 1,
            _ => out.push((p.name.clone(), 1)),
        }
    }
    out.into_iter().map(|(name, n)| if n > 1 { format!("{name} ×{}", bucket(n)) } else { name }).collect()
}

/// Run lengths rounded so jitter of a tick or two does not show as a difference.
fn bucket(n: usize) -> String {
    match n {
        0..=3 => n.to_string(),
        4..=9 => "4-9".into(),
        10..=29 => "10-29".into(),
        _ => "30+".into(),
    }
}

pub fn order(a: &Session, b: &Session, window: f64) {
    section(&format!("Order up to {:.0} s after spawn; streams at their first send (- left only, + right only)", window / 1000.0));
    let (ca, cb) = (counts(a), counts(b));
    let streams: HashSet<&str> = ca.iter().chain(&cb).filter(|(_, c)| c.0 >= STREAM_MIN).map(|(n, _)| *n).collect();
    let (sa, sb) = (sequence(a, window, &streams), sequence(b, window, &streams));
    let (la, lb): (Vec<&str>, Vec<&str>) = (sa.iter().map(String::as_str).collect(), sb.iter().map(String::as_str).collect());
    if diff::print(&diff::lines(&la, &lb), 2) == 0 {
        println!("    same order");
    }
}

/// For each one-off client packet: the server packet just before it and the delay.
fn triggers(s: &Session) -> BTreeMap<&str, (&str, f64)> {
    let mut out = BTreeMap::new();
    let mut last_server: Option<&Packet> = None;
    let counts = counts(s);
    for p in &s.packets {
        if !p.from_client {
            last_server = Some(p);
        } else if counts[p.name.as_str()].0 < STREAM_MIN
            && let Some(trigger) = last_server
        {
            out.entry(p.name.as_str()).or_insert((trigger.name.as_str(), p.t - trigger.t));
        }
    }
    out
}

pub fn reactions(a: &Session, b: &Session) {
    section("Reaction: first send after the latest server packet (! = other trigger or >2x and >50 ms off)");
    let (ta, tb) = (triggers(a), triggers(b));
    for (name, &(trig_a, da)) in &ta {
        let Some(&(trig_b, db)) = tb.get(name) else { continue };
        let off = trig_a != trig_b || ((da - db).abs() > 50.0 && (da.max(db) > 2.0 * da.min(db)));
        let mark = if off { "!" } else { " " };
        println!("{mark} {name:<36} {da:>7.0} after {trig_a:<28} {db:>7.0} after {trig_b}");
    }
}

fn percentiles(s: &Session, name: &str) -> Option<[f64; 3]> {
    let times: Vec<f64> = s.client().filter(|p| p.name == name).map(|p| p.t).collect();
    let mut gaps: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    if gaps.len() < STREAM_MIN {
        return None;
    }
    gaps.sort_by(f64::total_cmp);
    let at = |q: f64| gaps[((gaps.len() - 1) as f64 * q) as usize];
    Some([at(0.1), at(0.5), at(0.9)])
}

pub fn cadence(a: &Session, b: &Session) {
    section("Cadence: gap between sends, ms p10/p50/p90");
    let (ca, cb) = (counts(a), counts(b));
    for name in ca.keys().filter(|n| ca[*n].0 >= STREAM_MIN || cb.get(*n).is_some_and(|c| c.0 >= STREAM_MIN)) {
        let fmt = |p: Option<[f64; 3]>| p.map_or(format!("{:>17}", "-"), |[x, y, z]| format!("{x:>5.0}/{y:>5.0}/{z:>5.0}"));
        println!("  {name:<36} {}   {}", fmt(percentiles(a, name)), fmt(percentiles(b, name)));
    }
}

pub fn fields(a: &Session, b: &Session) {
    section("Fields of one-off client packets, first of each (- left, + right)");
    let (ca, cb) = (counts(a), counts(b));
    for name in ca.keys().filter(|n| ca[*n].0 <= ONE_OFF_MAX && cb.get(*n).is_some_and(|c| c.0 <= ONE_OFF_MAX)) {
        let firsts = |s: &Session| s.client().find(|p| p.name == *name).and_then(dump);
        let (Some(da), Some(db)) = (firsts(a), firsts(b)) else { continue };
        if da == db {
            continue;
        }
        println!("  {name}");
        let (la, lb): (Vec<&str>, Vec<&str>) = (da.lines().collect(), db.lines().collect());
        diff::print(&diff::lines(&la, &lb), 1);
    }
}

macro_rules! dumper {
    ($($t:ident),* $(,)?) => {
        /// The decoded packet as pretty Debug text.
        fn dump(p: &Packet) -> Option<String> {
            $(if p.id == <packets::$t>::ID { return <packets::$t>::decode(&mut &p.body[..]).ok().map(|v| format!("{v:#?}")); })*
            None
        }
    };
}
acacia_proto::for_each_packet!(dumper);
