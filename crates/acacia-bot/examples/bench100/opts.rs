pub struct Opts {
    pub server: String,
    pub bots: usize,
    pub radius: i32,
    /// Cell side in chunks; must exceed 2 * radius + 1 so views never overlap.
    pub spacing: i32,
    /// First cell's chunk offset from 0,0 on both axes, clear of the spawn view.
    pub origin: i32,
    pub y: i32,
    /// Sent by each bot once spawned, `{x} {y} {z}` filled with its cell's centre; `none` when the
    /// harness moves the bots from the server console (it reads the `cell` lines).
    pub tp_cmd: Option<String>,
    /// Sent by each bot before the teleport (`none` by default), e.g. `gamemode creative @s`.
    pub setup_cmd: Option<String>,
    pub time_cmd: Option<String>,
    pub poll: f64,
    pub rate: f64,
    pub min_settle: f64,
    pub max_settle: f64,
    pub steady: f64,
    pub shards: Option<usize>,
}

impl Opts {
    pub fn parse() -> Result<Self, String> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let [server, bots, rest @ ..] = args.as_slice() else {
            return Err("usage: bench100 <server> <bots> [--key value]...".into());
        };
        let mut o = Opts {
            server: server.clone(),
            bots: bots.parse().map_err(|e| format!("bots: {e}"))?,
            radius: 10,
            spacing: 32,
            origin: 64,
            y: 200,
            tp_cmd: Some("tp @s {x} {y} {z}".into()),
            setup_cmd: None,
            time_cmd: Some("time query gametime".into()),
            poll: 5.0,
            rate: 0.1,
            min_settle: 300.0,
            max_settle: 900.0,
            steady: 90.0,
            shards: None,
        };
        let num = |k: &str, v: &str| v.parse::<f64>().map_err(|e| format!("--{k}: {e}"));
        let opt = |v: &str| (!v.is_empty() && v != "none").then(|| v.to_owned());
        for pair in rest.chunks(2) {
            let [k, v] = pair else {
                return Err(format!("{} needs a value", pair[0]));
            };
            let k = k.trim_start_matches("--");
            match k {
                "radius" => o.radius = num(k, v)? as i32,
                "spacing" => o.spacing = num(k, v)? as i32,
                "origin" => o.origin = num(k, v)? as i32,
                "y" => o.y = num(k, v)? as i32,
                "tp-cmd" => o.tp_cmd = opt(v),
                "setup-cmd" => o.setup_cmd = opt(v),
                "time-cmd" => o.time_cmd = opt(v),
                "poll" => o.poll = num(k, v)?,
                "rate" => o.rate = num(k, v)?,
                "min-settle" => o.min_settle = num(k, v)?,
                "max-settle" => o.max_settle = num(k, v)?,
                "steady" => o.steady = num(k, v)?,
                "shards" => o.shards = Some(num(k, v)? as usize),
                _ => return Err(format!("unknown option --{k}")),
            }
        }
        Ok(o)
    }

    /// Chunk coordinates of bot `i`'s cell, on a square grid.
    pub fn cell(&self, i: usize) -> (i32, i32) {
        let side = (self.bots as f64).sqrt().ceil() as usize;
        (
            self.origin + (i % side) as i32 * self.spacing,
            self.origin + (i / side) as i32 * self.spacing,
        )
    }
}
