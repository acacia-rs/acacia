# Java Edition support

One library, both editions: the same `Bot` drives a Bedrock or a Java server, and pathfinding, building and
survival logic are shared. Decisions settled with the user on 2026-11-07.

## Decisions

| Area | Choice | Why |
|---|---|---|
| Home | Inside acacia, behind a cargo feature | One library; swapping editions is a connect-time choice |
| API shape | One `Bot`; edition chosen at connect (`Connect::java(..)` / `Connect::bedrock(..)`); edition-only extras via `bot.bedrock()` / `bot.java()` → `Option<_>` | swarm, the JS package and pathfinding keep one concrete type; dispatch cost is negligible next to the network |
| Names | Generated neutral `Block` / `Item` enums with typed properties (`state.get(prop::Facing)`); strings parse in either edition's spelling | Code is edition-free; per-block state structs (azalea-style) rejected as too large, string properties rejected because property names differ (`pillar_axis` vs `axis`) |
| World storage | Each edition's world keeps its native state ids; the neutral state is a view | Geyser's Java→Bedrock map is many-to-one; physics and placement need the exact native state |
| Java version | Pin 26.3, bumped together with the Bedrock pin (26.50) | Same single-version rule as Bedrock; ViaVersion servers accept newer clients |
| Codecs | Own `acacia-java-proto`, stable Rust, network-free like `acacia-proto` | azalea's 26.3 branch needs nightly-2026-08-15, git deps block crates.io, and its layouts were wrong (Login game mode, LevelParticles) |
| Codec source | Packet ids and registries from the 26.3 server jar's data-generator reports; layouts hand-written, checked with `javap -p -c` on the codec classes, round-tripped against a vanilla server and client | Reports don't carry field layouts |
| Physics | Bit-exact against the vanilla client: port LocalPlayer/Entity movement from the unobfuscated 26.3 jar (doubles where Java uses doubles); judged per tick against traces recorded by a mixin in the acacia-java-bridge Fabric mod under Xvfb | Exact positions keep Grim-style anticheats quiet; same bar as the Bedrock side |
| Bedrock-typed accessors | `client()`, `movement()`, `world()` move to `bot.bedrock()` (2026-11-10); no shims | Nothing outside depends on them yet; the JS branch (`js/cancel`) and the bridge's pinned rev update when they next take this branch |
| Shared logic | Pathfinding, build and survival plan against edition-neutral world + a physics trait; costs (jump/sprint reach) come from the edition's physics | Movement envelopes differ between editions |
| Licence | MIT; Mojang's jar is read for behaviour, not copied; GPL sources (Pumpkin, ViaBedrock) only as reference | Same rule as DESIGN.md |

## Build order

Each step ships on its own; Bedrock behaviour stays unchanged throughout.

1. **Neutral enums.** Generate `Block`, `Item` and typed properties from both editions' data (Bedrock registry +
   26.3 reports; Geyser mappings for the joins). Add a compat shim, then migrate acacia crate by crate off
   string names (~1,700 sites across 214 files on 2026-11-07; the bridge is the only external user).
2. **Java client, read-only.** `acacia-java-proto`; TCP + zlib + AES-CFB8 transport; login → configuration →
   play; Java auth via acacia-auth (Minecraft-services XSTS). A bot that joins and fills a Java world from
   chunk and entity packets. Java collision shapes come from the jar, not BDS.
3. **Java physics.** Port and fit against client traces until bit-exact. Crate `acacia-java-physics`; recordings
   in its `tests/traces`, made with the bridge's `tools/move-trace.sh` (branch `java-physics-trace`).
4. **Backend split.** Move the ~97 `acacia-bot` files that touch Bedrock packets behind edition backends
   (movement, interaction, inventory). Java bots path, dig and place. Done 2026-11-10 for goto with digging and
   bridging (`acacia-bot` example `java_goto`); still Bedrock-only: threat perception (Java entities untracked),
   enchantments on Java stacks, entity actions, item use in the air, forms, boats, elytra flight.
5. **Inventory and crafting.** Java container clicks (state ids, carried item) behind the same API as
   Bedrock's ItemStackRequest. Own-inventory moves done 2026-11-10: `Bot`'s item moves become window-0 clicks
   predicted as `doClick` (`acacia-java-client` `clicks.rs`, bot `items/java_clicks.rs`); live, goto pulls a tool
   from the main inventory with 0 Grim alerts. Containers too (`OpenScreen` → `state.containers.open`; example
   `java_chest` opens, fills, empties and reopens a chest with 0 alerts). Still to do: crafting and workstation
   result slots, enchantment and custom-name hashes.
6. **JS package.** Expose the edition in `@acacia-rs` bindings.

## Test servers

- Vanilla 26.3 server jar for codec round-trips and behaviour.
- Real 26.3 client (Fabric, Xvfb, `acacia-java-bridge/tools/client-shot.sh` pattern) for physics traces.
- Paper + Grim as an anticheat gate: `tools/grim-gate.sh` drives `acacia-java-client`'s `java_gate` example over key
  scripts; judge by server corrections and Grim's verbose flags. `ACACIA_GATE_CHEAT=1` is the control that must fail.
  Other examples run through it behind a wrapper that keeps `<host> <port>` and drops the gate's other arguments
  (e.g. `java_goto` with `ACACIA_DIG`/`ACACIA_BRIDGE`).
