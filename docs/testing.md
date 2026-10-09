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
  BDS needs an online account for NetherNet even when it is offline (an offline login offers a
  self-signed identity, which only hosts like sapling accept):
  `BEDROCK_TRANSPORT=nethernet cargo run -p acacia-client --example afk -- 127.0.0.1:19160 @default 30`.
  In Git Bash, set `MSYS_NO_PATHCONV=1`, or it rewrites `/list` into a Windows path.
- `cargo run -p acacia-client --example ping -- play.example.net` pings a server; set `BEDROCK_PROXY` to go through a proxy.

## Strict mode

For testing a server with acacia. Normally the session decodes only the packets it needs and the decoder is
lenient, so a malformed or misplaced packet passes unseen. With strict mode on, every server packet is fully
decoded and each problem is reported as a `Violation` (and logged at `warn`); the session keeps running, so one
run lists everything. A test asserts that no violation arrived.

- Turn it on: `BotConfig { strict: true, .. }` (`BotEvent::Violation`), `ClientBuilder::strict(true)`
  (`Event::Violation`, not subject to the packet filter), or `BEDROCK_STRICT=1` for the `afk` example.
- `Reason::Decode`: the strict decoder rejected the packet (docs/proto.md, "Strict decoding"), or its id has no type.
- `Reason::Content` (bots only): a `LevelChunk`, `Subchunk` or cache blob whose chunk data does not decode.
- `Reason::Order`: a join-order rule (`session/strict.rs`). Add a rule only once it is verified against vanilla or BDS.

| Rule | Basis |
|---|---|
| second NetworkSettings / second ServerToClientHandshake | each is sent once per connection |
| sent before PlayStatus(LoginSuccess) | anything but NetworkSettings, the handshake, PlayStatus and Disconnect |
| StartGame before ResourcePackStack | the pack exchange ends with the stack |
| StartGame without JigsawStructureData before it | a vanilla client kicks itself (observed 2026-10) |
| PlayerSpawn before StartGame | the session has no player to spawn |
| second StartGame | one per connection; a dimension change is ChangeDimension |

The replayed BDS 1.26.52 join produces no violation (`crates/acacia-bot/tests/strict.rs`), and every packet of it
passes the strict decoder (`bundled_script_passes_the_strict_decoder` in acacia-testserver). A violation against
BDS itself is therefore an acacia schema bug: fix it in `tools/codegen/data/overrides.json`.

Not covered: the resource-pack exchange (hash mismatches, pack contents), compression and encryption
negotiation, unknown block ids inside a chunk that decodes, and anything a vanilla client checks that the
schema does not express.

## Movement physics

Physics is checked against BDS itself: strict server-authoritative movement sends a `CorrectPlayerMovePrediction`
for every tick our position is off by more than 0.0001, so each correction is a disagreement to explain.

- `tools/bds.ps1 -Strict` starts BDS with that profile (plain `tools/bds.ps1` restores the default movement settings).
- `tools/physics/fuzz.sh <rounds> [seed]` builds random terrain on a pad (water, lava, slime, honey, ladders, scaffolding,
  powder snow...) and drives random inputs over it, recording a trace (`.testserver/traces/*.btrc`: the server's movement
  packets plus every input we sent). It prints the seed; `FUZZ_BOTS=n` runs n offline bots in parallel.
- `tools/physics/drill.sh <names>` runs scripted drills (`crates/acacia-bot/examples/drills.rs`), exact control
  sequences that isolate one variable.
- `cargo run -p acacia-bot --example flight -- <server> <name> [rounds]` drills creative flight (crates/acacia-physics
  README "Flight"); on testbox use its own strict instance (`BDS_DIR=work/bds-flight BDS_PORT=19178 tools/testbox-bds.sh`,
  with the `-Strict` keys above set in its server.properties).
- `cargo run -p acacia-bot --example dimensions -- <server> <name> [tp|portals|all]` drills dimension travel
  (docs/DESIGN.md "Dimension travel"): overworld, Nether and End by command, then by a Nether portal and End portal
  blocks, printing each arrival's and walk's corrections and the distance from the server's own position.
- `cargo run -p acacia-bot --example replay -- <trace> [--verbose]` replays a trace offline through the current code
  and reports server corrections we disagree with (`--correction-tolerance`, default 0.001) and ticks that differ from
  what the live bot did. "Diverged from the recording" means the replay would send other inputs than were recorded:
  rules that change our own input flags (sprint, swim, jump starts) can only be judged by a fresh live run.
- `crates/acacia-bot/tests/trace_regression.rs` replays the regression set in `crates/acacia-bot/tests/traces`
  (gzipped bot traces from the local BDS) and fails if a trace gains mismatches.
- Fuzz and drills need an online account in `./.tokens` of the checkout they run from (`@default`, or `ACCOUNT=@name`).

The loop:
1. Fuzz on a fresh seed; replay with `--verbose` and trace a mismatch with `RUST_LOG=acacia_bot=trace`.
2. Work out the rule from the numbers (back the server's velocity out of its positions; compare our step from the
   server's previous position, not from ours, or a stale offset looks like a per-tick error).
3. Wrap the candidate in an env toggle (`if std::env::var_os("HYP_X").is_some() { new } else { current }`) and run
   `tools/physics/hyp.sh "" HYP_X=1` over the regression set plus recent traces: it must fix and not regress.
4. A rule backed by one sample gets a drill that varies its parameter (and nothing else can fire: a honey column hid
   the single block's top threshold). Replay the drill under the toggle.
5. Make the rule unconditional, comment its evidence, list it in `crates/acacia-physics/README.md` if it departs from
   bedsim, add a trace that pins it to the regression set, and confirm with a fresh live fuzz.

Not every correction is physics, and the replay leaves out those the client could not have predicted:

- BDS updates per-tick state (the powder snow freeze) once per server tick, so when the network bunches our inputs a
  tick runs on a value reported later. A correction within two ticks of an input whose freeze differs from the one
  the server then reported is not counted; a vanilla client is corrected there too. `tools/physics/dupstamp.sh
  <trace>` and `tools/physics/freeze.sh` show those ticks.
- A correction for a tick simulated in a chunk not received yet (just after a long teleport) is not counted.
