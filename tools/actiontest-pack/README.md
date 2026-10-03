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
ridden; the next build removes them. The pig is saddled; the trader's llamas are removed.

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
