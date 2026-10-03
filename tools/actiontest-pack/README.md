# actiontest behaviour pack

Builds a test scene around every player that joins (2 s after the initial spawn), gives the items
for `examples/actions.rs`, and reports in chat and the BDS console (`ACTIONTEST ...`, needs
`content-log-console-output-enabled=true`):

- `ACTIONTEST scene base=x,y,z crafting_table=x,y,z ...`: block positions, sent 4 s after building
  (after the hunger effect has run out).
- `ACTIONTEST sign ["front","back"]`: whenever the scene sign's text changes.
- `ACTIONTEST inv [...]`: the server's view of the sender's stacks, on `/scriptevent actiontest:inv <item>`
  (empty item: all). BDS does not resend slots after crafts, pickups or signing, so this is the check.

Each join rebuilds the scene and replaces the inventory, so runs are repeatable. It sets night
(18000, daylight cycle off), turns off mob spawning and spawn spread, gives 30 levels and a short
strong hunger effect. Items come from `/give` and `/enchant`: BDS never sends the client inventory
changes made by scripts (`Container.addItem`).

Layout (offsets from the feet block, x east, z south; `@` = player):

```
       x-2         x-1          x         x+1     x+2       x+3        x+4..x+13
z-2    crafting    furnace      brewing   enchant anvil                water, z-3..z+3,
z-1                trader                 boat                         2 deep
z 0    stone                    @                 bed foot  bed head
z+1                farmer                 pig
z+2    grindstone  stonecutter  smithing  loom    sign (front faces north, towards @)
```

The "farmer" does not keep its job: it takes one at a nearby station (seen as shepherd and cleric,
also with a composter next to it) and cannot trade while between jobs, so the villager check
retries. `/scriptevent actiontest:ent <type>` reports the scene's entities of a type (position,
profession `variant`, sleeping).

Entities carry the tag `actiontest`, slowness 255, and are put back in place every second unless
ridden; the next build removes them. The pig is saddled. The trader's llamas are removed whenever
they spawn: they arrive ticks after the trader, and the boat pulled them in until both seats were
taken, which made the bot's mount fail silently.

Entity right-clicks log `ACTIONTEST interact before ...` (target, player state, held item, the
target's riders) and `interact after <type>`. The before event fires once the click passes the
transaction checks; the after event only if the entity offered an interaction. While a player
glides, every tick logs `ACTIONTEST glide <tick> <feet> v <velocity> rot <pitch>,<yaw>`: the server's own
glide, to line up against the bot's `auth input` trace: run the glide with
`RUST_LOG=acacia_bot::movement=trace`, then `py -3 tools/glide-compare.py <run>.bot.log <run>.bds.log`.

## Running (testbox)

```
tools/testbox-bds.sh install      # BDS 1.26.52.3 Linux in ~/work/bds-actions, :19170, flat world, this pack
tools/testbox-bds.sh start
REMOTE_DIR=work/bc-livetest tools/remote-run.sh cargo run --release -p acacia-bot --example actions -- 127.0.0.1:19170 ActionBot
tools/testbox-bds.sh log 100      # ACTIONTEST lines, script errors
tools/testbox-bds.sh stop
```

`ONLY=craft,write_sign` (`remote-run.sh env ONLY=craft cargo run ...`) runs a subset; a third argument
`idle` runs the bot without physics. Results: docs/research/live-action-tests.md.

One full run on a fresh world: `tools/live-actions.sh [idle]`. Repeated runs (flake hunts, before a
merge): `[ONLY=..] [RUST_LOG=..] [LOG_GREP='interact|dismounted'] tools/live-repeat.sh <runs> [idle]`.
It syncs and builds once, then loops on testbox (`tools/live-loop.sh`) on both instances
(`bds-actions` :19170, `bds-actions2` :19172) in parallel. It streams `<instance>/<run> PASS|FAIL`
lines and ends with a per-check tally. Failing runs keep their bot and BDS logs in
`$REMOTE_DIR/live-repeat/<instance>/`.
