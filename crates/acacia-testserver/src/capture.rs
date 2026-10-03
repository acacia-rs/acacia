//! Reads tools/mitm captures (format: tools/mitm/src/record.rs) into sessions.

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

/// Every session in the file, split at each `login` event.
pub fn read(path: &str) -> Result<Vec<Session>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut sessions: Vec<Session> = Vec::new();
    // Packets before a Login (network settings) belong to the session it starts; times are absolute until then.
    let mut pending: Vec<Packet> = Vec::new();
    let mut open = false;
    let mut zero = 0.0;
    for (n, line) in text.lines().enumerate() {
        let v: Value = serde_json::from_str(line).map_err(|e| format!("{path}:{}: {e}", n + 1))?;
        let t = v["t"].as_f64().unwrap_or(0.0);
        match v["event"].as_str() {
            Some("login") => {
                let packets = pending.drain(..).map(|p| Packet { t: p.t - zero, ..p }).collect();
                sessions.push(Session { packets, spawned: None });
                open = true;
            }
            Some("spawned") => {
                if let Some(s) = sessions.last_mut() {
                    s.spawned = Some(t - zero);
                }
            }
            Some("closed") => open = false,
            Some(_) => {}
            None => {
                let name = v["name"].as_str().unwrap_or("?").to_owned();
                // The Login line precedes its `login` event; it starts the session's clock.
                if name == "Login" {
                    zero = t;
                    continue;
                }
                let body = hex::decode(v["raw"].as_str().unwrap_or_default()).map_err(|e| format!("{path}:{}: {e}", n + 1))?;
                let id = v["id"].as_u64().unwrap_or(0) as u32;
                let packet = Packet { t, from_client: v["dir"] == "C>S", id, name, body };
                match sessions.last_mut() {
                    Some(s) if open => s.packets.push(Packet { t: t - zero, ..packet }),
                    _ => pending.push(packet),
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
