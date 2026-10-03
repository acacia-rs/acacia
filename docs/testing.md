# Testing

- `cargo test --workspace` runs unit tests, byte-exact packet fixtures and differential auth tests.
- Local server: `tools/bds.ps1` starts BDS from `.testserver/bds` (offline, RakNet, port 19140).
  Download BDS ≤ 26.5x into `.testserver/bds` first, because 26.60 removes RakNet. In
  `server.properties`, set `online-mode=false`, `allow-list=false` and `transport=raknet`.
- `cargo run -p acacia-client --example afk -- 127.0.0.1:19140 Bot 10`: joins one bot and prints a per-packet histogram.
- `cargo run --release -p acacia-client --example swarm -- 127.0.0.1:19140 10 15` joins N bots at
  once and reports spawn p50/p95 and failures. BDS allows 10 players by default.
- `cargo run -p acacia-auth --example device_login` signs in with a real account (device code) and caches tokens in `./.tokens`.
- `cargo run -p acacia-client --features socks --example afk -- <server> @default 60` runs an online bot.
  Set `BEDROCK_PROXY`, `BEDROCK_CMD="/list;hi;!respawn"`, `BEDROCK_AUTO_RESPAWN=1` and
  `BEDROCK_TRANSPORT=raknet|nethernet` (default auto) as needed.
- NetherNet: a second BDS copy in `.testserver/bds-nn` with `transport=nethernet` and `server-port=19160`.
  NetherNet needs an online account even when the server is offline:
  `BEDROCK_TRANSPORT=nethernet cargo run -p acacia-client --example afk -- 127.0.0.1:19160 @default 30`.
  In Git Bash, set `MSYS_NO_PATHCONV=1`, or it rewrites `/list` into a Windows path.
- `cargo run -p acacia-client --example ping -- play.example.net` pings a server; set `BEDROCK_PROXY` to go through a proxy.
