//! Reads acacia-mitm captures (format: crates/acacia-mitm/src/record.rs) into sessions.

use std::collections::HashMap;

use serde_json::Value;

pub struct Packet {
    /// ms since the session's Login.
    pub t: f64,
    pub from_client: bool,
    pub id: u32,
    pub name: String,
    pub body: Vec<u8>,
}

pub struct Session {
    pub packets: Vec<Packet>,
    /// ms since Login, when the server said PlayerSpawn.
    pub spawned: Option<f64>,
}

impl Session {
    pub fn client(&self) -> impl Iterator<Item = &Packet> {
        self.packets.iter().filter(|p| p.from_client)
    }
}

/// One connection's place in the file while it is read.
#[derive(Default)]
struct Reading {
    /// Packets before a Login (network settings) belong to the session it starts; times are absolute until then.
    pending: Vec<Packet>,
    /// Its session in the result, once logged in.
    index: Option<usize>,
    open: bool,
    zero: f64,
}

/// Every session in the file in login order. Lines are grouped by their `session`; captures from
/// before that field hold one connection at a time and are split at each `login` event.
pub fn read(path: &str) -> Result<Vec<Session>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut sessions: Vec<Session> = Vec::new();
    let mut readings: HashMap<Option<u64>, Reading> = HashMap::new();
    for (n, line) in text.lines().enumerate() {
        let v: Value = serde_json::from_str(line).map_err(|e| format!("{path}:{}: {e}", n + 1))?;
        let t = v["t"].as_f64().unwrap_or(0.0);
        let r = readings.entry(v["session"].as_u64()).or_default();
        match v["event"].as_str() {
            Some("login") => {
                let zero = r.zero;
                let packets = r.pending.drain(..).map(|p| Packet { t: p.t - zero, ..p }).collect();
                r.index = Some(sessions.len());
                r.open = true;
                sessions.push(Session { packets, spawned: None });
            }
            Some("spawned") => {
                if let Some(i) = r.index {
                    sessions[i].spawned = Some(t - r.zero);
                }
            }
            Some("closed") => r.open = false,
            Some(_) => {}
            None => {
                let name = v["name"].as_str().unwrap_or("?").to_owned();
                // The Login line precedes its `login` event; it starts the session's clock.
                if name == "Login" {
                    r.zero = t;
                    continue;
                }
                let body = hex::decode(v["raw"].as_str().unwrap_or_default()).map_err(|e| format!("{path}:{}: {e}", n + 1))?;
                let id = v["id"].as_u64().unwrap_or(0) as u32;
                let packet = Packet { t, from_client: v["dir"] == "C>S", id, name, body };
                match r.index {
                    Some(i) if r.open => sessions[i].packets.push(Packet { t: t - r.zero, ..packet }),
                    _ => r.pending.push(packet),
                }
            }
        }
    }
    Ok(sessions)
}

/// `path` or `path#N` (session N, from 0; negative counts from the end; default the last).
pub fn session(arg: &str) -> Result<Session, String> {
    let (path, index) = match arg.rsplit_once('#') {
        Some((p, i)) => (p, i.parse::<i64>().map_err(|e| format!("{arg}: {e}"))?),
        None => (arg, -1),
    };
    let mut all = read(path)?;
    let len = all.len() as i64;
    let i = if index < 0 { len + index } else { index };
    if !(0..len).contains(&i) {
        return Err(format!("{path} has {len} sessions; asked for {index}"));
    }
    Ok(all.swap_remove(i as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sizes(lines: &[&str]) -> Vec<(usize, Option<f64>)> {
        let path = std::env::temp_dir().join(format!("acacia-capture-{}-{}.jsonl", std::process::id(), lines.len()));
        std::fs::write(&path, lines.join("\n")).unwrap();
        let sessions = read(path.to_str().unwrap()).unwrap();
        std::fs::remove_file(&path).unwrap();
        sessions.iter().map(|s| (s.packets.len(), s.spawned)).collect()
    }

    #[test]
    fn interleaved_connections_are_split_by_their_session() {
        let got = sizes(&[
            r#"{"dir":"C>S","id":193,"name":"RequestNetworkSettings","raw":"","session":0,"t":1}"#,
            r#"{"dir":"C>S","id":193,"name":"RequestNetworkSettings","raw":"","session":1,"t":2}"#,
            r#"{"dir":"C>S","id":1,"name":"Login","session":1,"t":10}"#,
            r#"{"event":"login","session":1,"t":10}"#,
            r#"{"dir":"C>S","id":1,"name":"Login","session":0,"t":20}"#,
            r#"{"event":"login","session":0,"t":20}"#,
            r#"{"dir":"S>C","id":10,"name":"SetTime","raw":"00","session":1,"t":30}"#,
            r#"{"event":"spawned","session":1,"t":35}"#,
            r#"{"event":"closed","session":0,"t":40}"#,
            r#"{"dir":"S>C","id":10,"name":"SetTime","raw":"00","session":1,"t":50}"#,
        ]);
        assert_eq!(got, [(3, Some(25.0)), (1, None)]);
    }

    #[test]
    fn captures_without_a_session_split_at_each_login() {
        let got = sizes(&[
            r#"{"dir":"C>S","id":1,"name":"Login","t":10}"#,
            r#"{"event":"login","t":10}"#,
            r#"{"dir":"S>C","id":10,"name":"SetTime","raw":"00","t":30}"#,
            r#"{"event":"closed","t":40}"#,
            r#"{"dir":"C>S","id":193,"name":"RequestNetworkSettings","raw":"","t":50}"#,
            r#"{"dir":"C>S","id":1,"name":"Login","t":60}"#,
            r#"{"event":"login","t":60}"#,
        ]);
        assert_eq!(got, [(1, None), (1, None)]);
    }
}
