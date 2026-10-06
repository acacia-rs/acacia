# acacia-proto and the code generator

`crates/acacia-proto/src/generated/` is produced by `tools/codegen` from the vendored
minecraft-data `tools/codegen/data/protocol.json` + `version.json`. Never edit the generated files.

## Regenerate

`cargo run -p acacia-codegen`. The output is deterministic and is formatted with `rustfmt --edition 2024`.

Local corrections to minecraft-data go in `tools/codegen/data/overrides.json`: each `types` entry replaces or
adds a schema type before generation, and each `patches` entry replaces one container field at a JSON pointer
(the field name must match, so a bump that moves the field fails codegen). Give each one a reason here:
- `available_commands` parameter `value_type` is a plain `lu16`, not a mapper. With `enum_type` `enum`,
  `soft_enum` or `suffixed` it is an index into that list, and minecraft-data's type numbers are stale for
  1.26.52: BDS sends 56 for string parameters (`name`), 64 block position (`begin`, `end`), 65 position
  (`spawnPos`), 68 message, 74 json (`components`), 84 block states and 90 command, where the schema has
  44, 52, 53, 55, 62, 71 and 75.
- `available_commands` parameter `enum_type` gains 2064 `chained_subcommand`: BDS sends it for `/execute chainedCommand`.
- `available_commands` enum `constraint` gains 3 `allow_aliases`: BDS sets it on every `minecraft:`-prefixed
  value of the `Block` enum. The name is Mojang's `RequiresAllowAliases` as remembered, not checked in bdsre.

  All three were found by `bundled_script_passes_the_strict_decoder` (acacia-testserver), which holds the evidence.
- `EnchantOption`: `option_id` is an unsigned varint (gophertunnel `EnchantmentOption`, Cloudburst);
  minecraft-data has `zigzag32`. (`cost` is a varint too, but equals the `u8` below 128.) BDS answered the zigzag-misread id with status 37 (FAILED_TO_ENCHANT).
- Not an override: `craft_grindstone_request.recipe_network_id` stays `li32` although gophertunnel writes a varint;
  BDS 1.26.52 answers the varint form with PacketViolationWarning (Malformed) and drops the client. Mojang's docs
  type it `ItemStackNetIdVariant`: it carries the input stack's id, not a recipe id.
- Not an override: `boss_event` stays flat (id, `u8` type, title, filtered title, `lf32` progress, `u8` colour, `u8` overlay
  for every event type). Older protocols switched on the type and ShowBar had an `li16` screen darkening; BDS 1.26.52.3's
  ShowBar for a wither is 33 bytes and the flat layout reads all of them (`tests/roundtrip.rs`), as gophertunnel now
  writes it. BDS sends nothing but ShowBar (once a second, and none while the player is out of range), so the other
  types have no capture behind them.

Fixtures come from the same schema, so they can't catch schema errors: check against a real server
(BDS) and the Geyser lab (`tools/lab.ps1`).

## Bump to a new protocol version

1. Look up `bedrock["<ver>"].protocol` in minecraft-data `data/dataPaths.json`. The protocol may live under an older version's directory.
2. Download that `protocol.json` and `version.json` into `tools/codegen/data/`, then regenerate.
3. If codegen panics, it names the unsupported construct. Extend `lower.rs`/`lower_switch.rs`, or add a hand-written type in `src/manual/`.
4. Fix compile errors in hand-written tests, which usually come from renamed fields.
5. Refresh the fixtures: `npm i bedrock-protocol` (a release that supports the version) in some dir `D`, then run
   `BP_DIR=D node crates/acacia-proto/tests/fixtures/gen.mjs` and bump `VERSION` in the script.
6. Run `cargo test -p acacia-proto`.

## Codegen pipeline

`schema.rs` (JSON → AST) → `lower.rs` + `lower_switch.rs` (resolve names, hoist inline items, bind `compareTo`/`count` to local variables) → `emit_read.rs` / `emit_write.rs` / `emit_items.rs` → `output.rs` (files grouped by top-level type, size-chunked into `part_NN.rs`).

## Type mapping

| ProtoDef | Rust |
|---|---|
| varint / varint64 / varint128 | `u32` / `u64` / `u128` |
| zigzag32 / zigzag64 | `i32` / `i64` |
| u8 i8 lu16 li16 lu32 li32 lu64 li64 lf32 lf64 (+ big-endian u16…f64) | matching primitive |
| bool | `bool` |
| pstring (`string`, `ShortString`, `LittleString`, `LatinString`) | `String`. Invalid UTF-8 is decoded lossily, never as an error |
| buffer (`ByteArray`, …), restBuffer | `bytes::Bytes` |
| uuid | `manual::Uuid` (16 raw wire bytes; `Display` matches bedrock-protocol's string) |
| byterot | `f32` degrees (`byte * 360/256`) |
| nbt | `nbt::Raw<nbt::Network>` (see "Lazy NBT") |
| lnbt / nbtLoop | `nbt::Nbt` (LittleEndian flavour) / `Vec<nbt::Nbt>` (Network); the tree and codec are the `acacia-nbt` crate, re-exported as `acacia_proto::nbt` |
| container (top-level or packet) | struct with `read`/`write`; packets also `impl Packet` |
| container (inline) | hoisted struct `<Parent><Field>`, decoded in place |
| array (countType / fixed / count field) | `Vec<T>` / `[T; N]` / `Vec<T>` (the count field stays a normal field) |
| option, optionalOnRemaining | `Option<T>` |
| encapsulated | `Option<T>`: length 0 is `None`, as in JS |
| mapper | open enum, `Copy + Eq + Hash`, with an `Unknown(i64)` fallback. A mapping literally named `unknown` becomes `UnknownValue` |
| switch with exactly one non-void arm | `Option<T>` |
| other switches | enum `<Parent><Field>`, one variant per key plus `Default` (unit if the default is void). The discriminant stays a separate field, and encoding writes only the variant payload |
| anonymous switch (fields merged in JS) | field named `content` |
| bitflags | newtype over the repr with `UPPER_SNAKE` consts plus `contains`/`insert`/`remove` |
| bitfield | struct, packed MSB-first |
| named array/option types (e.g. `Links`) | `pub type` alias |

Keywords are escaped as `r#type`. Packets live in `packets`, shared types in `types`, and each item exists once.
`for_each_packet!(m)` expands to `m!(Login, PlayStatus, …)`, listing every packet type in id order.

## Manual types (`src/manual/`)

- `Uuid`.
- `shield_item_id()` / `set_shield_item_id()`: a process-wide value that replaces minecraft-data's `/ShieldItemID` switch key. Items whose network id equals it carry an extra `blocking_tick`. It defaults to 387 (vanilla). The session layer should set it from the `item_registry`/`start_game` item list.

## Tests

- `tests/fixtures.rs`: every packet in `tests/fixtures/packets/*.hex` must decode and re-encode byte-exactly. All 246 packets are covered, and the client-relevant ones are asserted by name. The generator `gen.mjs` draws random schema-valid values, encodes them with bedrock-protocol, and keeps only samples that JS itself round-trips.
- `tests/semantic.rs`: decoded field values are compared with the JS-decoded `*.json`.
- `tests/roundtrip.rs`: hand-built values, the header and subclient bits, and error context.
- `tests/strict.rs`: each leniency below is accepted by `decode` and rejected by `decode_strict`.

## Lazy NBT

A packet-level `nbt` field decodes to `nbt::Raw<Network>`: the decoder finds the tag's end and keeps its
bytes, and `.decode()` builds the tree when someone wants it. `Raw::from(nbt)` goes the other way.
In the recorded BDS join, NBT was 74% of decoding `StartGame`, 78% of `JigsawStructureData` and 42% of
`ItemRegistry`, and almost none of it is read; keeping the bytes made those packets 4.9, 5.9 and 1.4 times
faster to decode. Encoding one back is a copy.

Item NBT (`lnbt`, inside item extra data) stays an eager `nbt::Nbt`: the documents are small and the bot
reads most of them.

## Decode cost

`cargo run --release -p acacia-testserver --example decode_cost` decodes every packet of the recorded BDS
join and prints time and allocations per type; with a packet name it loops on that type for a profiler.
Decoding is bound by allocation, one per string and per list. A sampled profile of `CraftingData` (62,000
allocations for 1 MB) has 39% of its time in heap allocation and 18% in freeing, against about 20% in
the generated readers themselves.

Because the cost is the allocator, the allocator is the cheapest fix: the same tool built with
`--features mimalloc` decodes the whole join in 5.4–5.8 ms against 12.5–13.2 ms on the Windows system heap
(`CraftingData` 2.3×, `AvailableCommands` 2.6×, `ItemRegistry` 1.7×, `PlayerList` 6×). That is a
choice for a binary to make with `#[global_allocator]`, never for these libraries. The `acacia-mitm` CLI
makes it (its default `mimalloc` feature).

That table is every packet, not what a client pays. A bot join decodes none of the three largest
(`CraftingData` only on the first recipe lookup, `AvailableCommands` and `BiomeDefinitionList` never). What
it does pay for is the same packet decoded by several readers, so a reader that wants one field should
not build the packet:

- `manual::shield_item_id_in_registry` scans an `ItemRegistry` body for the shield's id (about 25 µs
  against 500 µs for the decode); the session uses it.
- The session reads only `StartGame`'s runtime entity id (`read_runtime_entity_id`).

Still open: the bot decodes `ItemRegistry` once and `StartGame` and each `LevelChunk` three times, once
per state module.

## Strict decoding

`decode` is lenient so one odd field does not cost a bot the packet. `RawPacket::decode_strict::<T>()` and
`strict::check(&raw)` (any packet, by id) decode the same way and then fail on:

| Error | What `decode` does with it |
|---|---|
| `TrailingBytes(n)` | ignores bytes after the body |
| `Lenient(UnknownEnum)` | maps the value to `Unknown(i64)` |
| `Lenient(Bool)` | reads any non-zero byte as `true` |
| `Lenient(Utf8)` | replaces invalid UTF-8 |
| `Lenient(TruncatedList)` | ends a `maybeIncompleteArray` (`PlayerList` records) early |
| `Lenient(BlockRest)` | drops bytes left inside an `encapsulated` block (item `extra`, login tokens) |
| `UnknownPacket(id)` | nothing: no type exists for the id, so nobody decodes it (`check` only) |

The decoder notes a leniency in a thread-local on the cold path, so `decode` pays nothing for it; only the
first one of a packet is reported. Not checked: overlong varints, a `restBuffer` field (it takes whatever is
left), and an integer switch discriminant with no matching case when the default is void (the schema cannot
tell a payload-free value from an unknown one). Who calls this: docs/testing.md, "Strict mode".

## Known gaps / deliberate divergences

- In network NBT, IntArray/LongArray elements are zigzag varints (Mojang/gophertunnel). prismarine-nbt writes fixed-width
  ints there, so the fixtures avoid those tags.
- A root NBT `End` tag is 1 byte, as Mojang writes it. prismarine-nbt's varint flavour also writes an empty name there.
- `encapsulated` payloads are bounded by their length prefix (JS reads past it). Trailing bytes after a packet body are ignored (see "Strict decoding").
- The shield item id is global, not per-connection. Clients that connect to servers with different registries in the same process share it.
- Switch keys that can never match their discriminant are dropped with a codegen warning. There are none in 1.26.51.
- Structs don't derive `Default`, so you must spell out every field when you build a packet.
- The wire layout follows minecraft-data as written. For example, `player_auth_input.input_data` is an array of mapper values, exactly as bedrock-protocol encodes it.
