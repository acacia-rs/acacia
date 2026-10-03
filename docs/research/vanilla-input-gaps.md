# PlayerAuthInput: where the bot differs from the vanilla client

Source: mitm capture of Minecraft for Windows 1.26.52.3 against local BDS (2026-10-01, two sessions,
4803 inputs), replayed with `mitm_trace` + `replay --resync`. Code refs are `crates/acacia-bot/src/movement/`.

Fixed on branch `physics-fuzz`: `WantUp` with every jump, `WantDown` with every sneak,
`StartSwimming`/`StopSwimming` edges, `StopSprinting` on the tick a swim ends.

Still different (not physics; left to the client-fingerprint work):

| Field | Vanilla | Bot | Where |
|---|---|---|---|
| `BlockBreakingDelayEnabled` | set on every tick | never | `auth_input.rs` flag list |
| `play_mode` | `Screen` (2) | `Normal` (0) | `auth_input.rs` |
| `interaction_model` | `Touch` (0) | `Crosshair` (1) | `auth_input.rs` |
| `UpLeft`/`UpRight`/`DownLeft`/`DownRight` | never sent (keyboard) | sent with diagonal input | `auth_input.rs` |
| `HandledTeleport` after a respawn | not sent | sent (respawn is queued as a teleport) | `mod.rs` `Respawn` → `queue_teleport` |
| `tick` while dead | keeps counting (gap of ~42 ticks across a death) | frozen until respawn | `bot.rs` returns before `movement.tick` while dead |
| `interact_rotation` | lags `pitch`/`yaw` slightly (render rotation) | equal to `pitch`/`yaw` | `auth_input.rs` |
| `SneakCurrentRaw` | stays set while the key is held even after `StopSneaking` (jump held in water) | follows `SneakDown` | `auth_input.rs` |

Matches already: `camera_orientation` formula, `input_mode` `Mouse`, `move_vector`/`raw_move_vector`,
`analogue_move_vector` zero.
