# acacia-nbt

Minecraft: Bedrock Edition NBT as an owned value tree, in both wire flavours:

| Flavour | Ints and lengths | Where |
|---|---|---|
| `Network` | zigzag varints, varint string lengths | packets (minecraft-data `nbt`) |
| `LittleEndian` | fixed-width little endian, `u16` string lengths | item extra data, world storage (`lnbt`) |

`read`, `skip` and `write` take the flavour as a type parameter; `Raw<F>` is a tag kept as its bytes and
decoded on demand. `acacia-proto` re-exports the crate as `acacia_proto::nbt`, with `DecodeError` results
and strict-mode reporting added.

Compounds keep their entries in wire order and lists keep their element tag, so a document that was read
re-encodes to the same bytes (`tests/corpus.rs` holds that for every corpus document).

## Limits

- Nesting stops at `MAX_DEPTH` (`Error::TooDeep`).
- A count never allocates beyond what the remaining input could hold.
- A list of `End` tags with a length is rejected (`Error::EndList`): its elements take no bytes, so the
  count would not be bounded by the input. An empty one is what vanilla writes for an empty list.
- Invalid UTF-8 is replaced, not refused; `read_lossy` says when that happened.
- `skip` and `Raw::read` accept exactly the documents `read` accepts.

## Writing

`write` cannot fail, so the tree has to be well formed: list items match the list's tag, and a compound has
no `End` entry (both are debug assertions). A string longer than the flavour's length prefix can hold is
cut at a character boundary.

## Corpus

`corpus/network` and `corpus/le` hold real documents, one root tag per file, named `<kind>-<n>.nbt`. Tests,
benches, the profiling example and the fuzz seeds all read them.

`cargo run -p acacia-nbtcorpus` regenerates them from the recorded BDS join
(`acacia-testserver/fixtures`) and the captured chunks (`acacia-world/tests/fixtures`): up to 24
documents per kind, spread over the sizes seen. Neither source has a block entity, so that kind is missing.

## Benchmarks

`cargo bench -p acacia-nbt [-- <filter>]` measures read, skip and write throughput per flavour and kind,
for example `-- le/item/read`.

## Profiling

```
cargo build -p acacia-nbt --profile profiling --example profile
samply record target/profiling/examples/profile <read|skip|write> <network|le> [seconds]
```

The example first prints throughput and what one pass allocates, then loops for the profiler.

## Performance

Reading is bound by allocation, not by parsing: `skip` walks the same bytes about ten times faster than
`read`. What the profile led to:

- Keys and string values are `Str` (`compact_str`), inline up to 24 bytes. Three quarters of the corpus's
  allocations were strings and 82% of those fit, which took a network document from 93 allocations to 32.
  The root name stays a `String`: it is almost always empty, and an empty `String` does not allocate.
- `Value` is four words (`tests/limits.rs` holds that), so list items and compound entries stay small.
- What is left is one allocation per container, plus freeing the tree. `Raw` avoids all of it for NBT
  nobody reads: it finds the end of the tag, keeps the bytes (inline up to 38, one allocation beyond) and
  decodes on demand. The inline case matters: an item registry carries a tiny document per item, and
  allocating for each was 23% of decoding it. It reads about five times faster than `read`; `acacia-proto` uses it for
  packet-level NBT (docs/proto.md, "Lazy NBT").
- `Raw` still validates strings, so strict decoding can report bad UTF-8 without a tree. Keys are nearly
  all short ASCII, so that check tries `is_ascii` before `from_utf8`.

## Fuzzing

`fuzz/run.sh <decode|tree> [seconds]` (nightly, `cargo install cargo-fuzz`):

- `decode`: arbitrary bytes through `read` and `skip` in both flavours.
- `tree`: generated well-formed trees through `write`, then `read` and `skip`.

Both check the properties in `tests/common/props.rs`. `tests/corpus.rs` runs the `decode` property over
seeded mutations of the corpus, so a stable `cargo test` covers it too; CI runs each target for a minute.
A crash lands in `fuzz/artifacts/<target>/`; turn it into a case in `tests/limits.rs`.
