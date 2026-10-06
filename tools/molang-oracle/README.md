# molang-oracle

Asks a real Bedrock Dedicated Server what Molang expressions evaluate to, so `acacia-molang`'s
expected values are observed, not read out of the documentation. No player joins; a run of a few
hundred cases takes about ten seconds.

```
py -3 tools/molang-oracle/oracle.py crates/acacia-molang/tests/oracle/*.cases
```

Each `<name>.cases` gets a `<name>.bds` beside it: a header with the BDS version, then one line per
case of `result <tab> case <tab> Molang errors BDS logged for it`. Commit both.

A cases file with a line `# engines: 1.13.0 1.18.10` is asked once per listed version, with the pack's
`min_engine_version` set to it, and writes `<name>@<version>.bds` files: Molang's rules depend on that
version. Each version is one more server run.

The server is `--bds DIR`, else `$MOLANG_BDS`, else `.testserver/bds-molang` (in this checkout, or in
the main checkout `../acacia` when run from a worktree): a copy of the test BDS
made for this tool, since every run replaces its `worlds/oracle`, its `molang_oracle` pack and six
`server.properties` keys (port 19180). The console output of the last run is `<DIR>/oracle.log`.

## Cases

One per line; `#` starts a comment line. Every case runs on its own entity, so variables never leak
between cases.

| Line | Runs | Result |
|---|---|---|
| `expr` | `expr` in an entity event's `set_property` | its value |
| `statements => expr` | `statements` as an animation controller state's `on_entry`, then `expr` as above | the value of `expr` |
| `truthy: complex` | `complex` as an animation controller transition | `1` if the transition was taken, else `0` |

`set_property` rejects assignment and some queries (`query.is_baby`), which is why statements go
through the controller.

## Results

- A number, as the shortest decimal that reads back as the same f32.
- `error`: BDS rejected the expression when it loaded the pack, so the event does not exist. The
  reason is in the third column.
- `unset`: the event ran but stored nothing (a string or a struct into the float property).
- `missing`: the server stopped before reporting the case; the tool exits 1.

A number with an error beside it was still evaluated: BDS logged the error. It did not always carry
on: reading an unknown variable ends the program with 0, so ask for a variable the program set
earlier or later (`v.r = 7; v.r = v.none + 5; => v.r`) to tell that from a 0.

## Vanilla corpus

```
py -3 tools/molang-oracle/corpus.py            # fetches Mojang/bedrock-samples at the pinned tag
```

writes every distinct Molang expression of the vanilla resource and behaviour packs to the
git-ignored `assets/molang/corpus.txt`, one per line, for `crates/acacia-molang/tests/corpus.rs`.
The packs are Mojang's (Minecraft EULA), so the corpus is never committed. `--samples DIR` reads an
existing checkout. An expression is any string holding a Molang prefix (`query.`, `math.`, ...), so
ones without a prefix are missed; `scripts.pre_animation` and `scripts.initialize` arrays are joined,
as the game runs each as one program.

## Query names

```
py -3 tools/molang-oracle/queries.py           # downloads the list at the pinned bedrock-samples tag
```

rewrites `crates/acacia-molang/src/queries.rs` from Mojang's `mojang-molang-queries.json`: the query
names, and the engine version after which each removed one is gone. Names and versions only; the
descriptions stay with Mojang. Rerun it for a new game version.

## Limits

- Values pass through a float property with the range ±1e30, so not-a-number and infinities come
  back as `-1e+30` or `1e+30`. Test them inside the expression (`v.r == v.r`).
- Strings can only be observed through comparisons, and a complex expression's value only as true or
  false: compare inside it (`truthy: v.c = 3; return v.c * 2 == 6;`).
- BDS has no loop limit: a case looping millions of times stalls the server until the timeout, and
  every case of that run comes back `missing`.
- Server-side Molang only. Client queries (animation time, bones, render state) cannot be asked.
- The pack's `min_engine_version` (`pack/manifest.json`) selects the Molang version BDS applies.
