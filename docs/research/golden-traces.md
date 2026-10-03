# Golden movement traces for the bedsim port — research (2026-09-30)

Goal: per-tick vanilla player state (pos, velocity, on_ground, in_water) to validate our Rust port of
oomph-ac/bedsim. Target: BDS 1.26.52.3, protocol 2193 (26.52 is a hotfix on the 1.26.50 protocol, per
[wiki](https://minecraft.wiki/w/Bedrock_Dedicated_Server_1.26.52.3)).

## TL;DR recommendation
1. **The real client is the oracle.** Every `PlayerAuthInput` carries the client's own simulated
   `Position`, `Delta` (velocity), `Tick`, `MoveVector` and flags incl. `HorizontalCollision` /
   `VerticalCollision` / `StartSwimming` / `StartJumping` ([gophertunnel packet](https://github.com/Sandertv/gophertunnel/blob/master/minecraft/protocol/packet/player_auth_input.go)).
   Record that stream through a proxy while a real client walks scripted routes. This is the same
   reference oomph/bedsim uses: its sim is checked live against client PAI.
2. **Use BDS as the second opinion.** Run local BDS in strict mode with a near-zero tolerance so every
   divergence produces a `CorrectPlayerMovePrediction` packet carrying the server's pos, delta, on_ground
   and tick. Our BDS already has `server-authoritative-movement-strict=true` and
   `player-position-acceptance-threshold=0.0001`. Also set `player-rewind-min-correction-delay-ticks=0`.
   Gotcha: the server "eases" toward the client pos and accepts the client pos when it is within a
   fraction of the threshold ([AntiCheatServer.md](https://github.com/Mojang/bedrock-protocol-docs/blob/main/additional_docs/AntiCheatServer.md)),
   so server state is not a pure simulation unless the threshold is tiny.
3. **Only if we need `in_water` and internal state:** use a LeviLamina plugin (Windows, **BDS 1.26.51**
   exactly) that hooks `ServerPlayerInputSystem::_tickPlayerMovement` and dumps the component state
   after each tick. See section 1.

## 1. BDS plugin loaders (server-side hooking)

| | LeviLamina | Endstone |
|---|---|---|
| Latest | v26.51.6 (2026-09-29) | v0.11.12 (2026-09-20) |
| BDS pinned | **1.26.51** (`tooth.json`: `github.com/LiteLDev/bds: 1.26.51`) | 1.26.51 ([PR #534](https://github.com/EndstoneMC/endstone/pull/534)) |
| 1.26.52.3 | Not confirmed. Hooks are resolved by offset, so a hotfix binary probably needs a new runtime-data release. Run 1.26.51 for this work. | Not confirmed |
| OS | Windows only. The Linux proposal was closed in Jan 2025; wine is the suggested workaround ([#1442](https://github.com/LiteLDev/LeviLamina/issues/1442)). A client build also exists ([install on client](https://lamina.levimc.org/user_guides/install_on_client/)). | Windows and Linux |
| Symbol source | `bedrock-runtime-data` (symbol/offset tables) | Byte-signature scan with lief, written to `src/bedrock/symbols/{linux,windows}.h`, with an optional `--pdb` path ([dump_symbols.py](https://github.com/EndstoneMC/endstone/blob/main/scripts/dump_symbols.py)) |

Links: [LeviLamina releases](https://github.com/LiteLDev/LeviLamina/releases),
[Endstone releases](https://github.com/EndstoneMC/endstone/releases).

**Reconstructed headers do name the movement pipeline.** I checked the LeviLamina `main` tree on 2026-09-30:
- `src/mc/entity/systems/ServerPlayerInputSystem.h` exports
  `MCAPI void _tickPlayerMovement(StrictEntityContext&, ServerPlayerMovementComponent&, EntityModifier<ServerPlayerCurrentMovementComponent, ...>)`.
  This is the best hook point: it is the per-PAI server simulation step.
- `ServerPlayerMovementComponent.h` has `addPlayerAuthInputPacket(...)` (MCAPI), plus the members
  `mServerHasMovementAuthority`, `mAcceptClientPosIfWithinDistanceSq`, `mQueuedUpdates` (deque of `MovementPackets`) and `IPlayerTickPolicy`.
- `Mob.h` has `$aiStep`, `$normalTick`, `jumpFromGround`, `calcMoveRelativeSpeed(TravelType)` and `getTravelType`.
  `TravelType` can give the in-water / lava / air state.
- `systems/movement/mob/MobMovement{Gravity,Friction,Drag,Speed,Climb,Levitate}.h`,
  `collision/swept_movement/SweptMovement.h`, `IMovementCorrection.h` (`advanceFrame` / `advanceLiveFrame`),
  `ReplayState*`, `client_rewind/ClientRewind.h`.
- Caveat: `ServerPlayerMovementSystem.h` is an empty struct, and many functions are `MCNAPI` (declared
  but not resolvable). Mojang's newer builds inline heavily. Pier's notes say LeviLamina 26.32 declares
  "about a third fewer non-virtual functions than 26.20" ([Pier 26.32.0](https://github.com/Maskviva/Pier/releases/tag/v26.32.0)).
  Check that every hook target is `MCAPI` in the exact version you pin.
- Endstone ships its own smaller header set (`server_player_movement_component.h`,
  `movement_correction_interface.h`, `provider/player_movement.h`), and each hook needs its own signature.

**Symbols from Mojang:**
- Windows BDS: the PDB was removed around 1.21.10 after a large community pushback
  ([feedback post](https://feedback.minecraft.net/hc/en-us/community/posts/27425253557389-Request-to-Continue-Providing-PDB-Files-in-Future-BDS-Versions)).
  Our local `.testserver/bds` (Windows, 1.26.5x) has **no .pdb**.
- Linux BDS: public builds are stripped. PMMP says "Mojang no longer publicly provide BDS builds with
  debugging symbols as of 1.21.40"; symbolled builds go only to partners
  ([PMMP docs](https://doc.pmmp.io/en/rtfd/developers/internals-docs/updating-minecraft-protocol.html)).
- Uncertain: exactly how LeviLamina produces its runtime data without public symbols. It is not documented.

**Existing per-tick loggers:** I found no public plugin that dumps server-simulated player state per
tick. The closest things are:
- oomph's `movement_sim` debug mode (proxy side; see [oomph debug.go](https://github.com/oomph-ac/oomph/blob/master/anticheat/player/debug.go)).
- BDS's `MovementCorrectionTelemetryComponent` header. It is internal telemetry, not exported.

## 2. Proxy recorders

- **CloudburstMC ProxyPass** ([repo](https://github.com/CloudburstMC/ProxyPass)):
  - Already uses the `Bedrock_v2193` codec, so it speaks current protocol. It was active in 2026-09.
  - The destination BDS must run with `online-mode=false`, because the proxy re-signs the login. That is
    fine for a local BDS.
  - `log-packets: true` / `log-to: file` writes packet logs. `PlayerAuthInputPacket` is in
    `ignored-packets` by default, so remove it from that list.
  - AGPL-3.0, Java.
- **Kas-tle/ProxyPass fork** ([repo](https://github.com/Kas-tle/ProxyPass)):
  - Supports `online-mode: true`, with Microsoft login on start and saved credentials, so it works with
    online servers.
  - Last commit 2026-07-27, so it may lag protocol 2193. Unverified.
- **gophertunnel**:
  - `CurrentProtocol = 2193`, `CurrentVersion = "1.26.50"`, with commits up to 2026-09-28. The latest
    tag is v1.62.0.
  - It has a documented MITM proxy pattern ([guide](https://www.mintlify.com/Sandertv/gophertunnel/guides/proxy)).
    It does full Xbox auth for the upstream connection (`auth.TokenSource`) and can listen with
    `AuthenticationDisabled` downstream.
  - Best fit: we get typed PAI structs, and bedsim's own tests import gophertunnel types.
  - oomph itself is a gophertunnel proxy, so it is also an option: run oomph in front of BDS with
    `movement_sim` debug. That gives us bedsim-vs-client diffs for free.
- **PrismarineJS bedrock-protocol** (Node): `relay` supports recording, active 2026-09-22 (v3.60.1).
  Not checked for 2193.
- Loopback: the Windows client moved to GDK at 1.21.120 and is no longer sandboxed
  ([MS Learn](https://learn.microsoft.com/en-us/minecraft/creator/documents/gdkpcprojectfolder?view=minecraft-bedrock-stable),
  [jaylydev](https://jaylydev.github.io/posts/bedrock-gdk/)). The UWP `CheckNetIsolation LoopbackExempt`
  step should no longer be needed. This is inferred, not tested.

## 3. Driving the real client

**mcpelauncher (minecraft-linux)** ([manifest releases](https://github.com/minecraft-linux/mcpelauncher-manifest/releases)):
- v1.8.4 (2026-09-17). The Qt6 builds "now support launching 26.50". I found no confirmation for 26.52.
- There is an open crash report for 1.26.21.1 ([#1872](https://github.com/minecraft-linux/mcpelauncher-manifest/issues/1872)).
- Support lags Mojang by weeks.
- It requires the game to be owned on Google Play and downloads the APK through the Google login
  ([docs](https://mcpelauncher.readthedocs.io/)). The Xbox sign-in happens in-game.
- **bedrock-mc/mcpelauncher-agent** ([repo](https://github.com/bedrock-mc/mcpelauncher-agent)):
  - A fork that adds an agent socket speaking JSON lines: `key` press/release/hold, `mouse_move`
    (look), `click`, `screenshot`, `fps` cap.
  - Input is injected like real keyboard and mouse input, with no game symbols, so it survives updates.
  - It needs a GPU; on Linux run it under `xvfb-run -a`. Their Linux path is "not tested yet", and
    macOS is the primary platform.
  - Very new (4 stars, pushed 2026-09-26). This is the most scriptable real-client path.
- Caveat: frame-rate-driven input timing means the tick alignment of keypresses is not exact.
  Record the PAI `InputData` and `MoveVector` as ground truth for the inputs, rather than trusting
  the script.

**Windows GDK client:**
- It is a normal Win32 process now. Scripting with `SendInput` or AutoHotkey works for keyboard and
  mouse. It needs a focused window, so use a dedicated VM or session.
- The LeviLamina client build and Latite ([repo](https://github.com/LatiteClient/Latite)) are DLL
  injection options. A client-side LeviLamina mod could log `LocalPlayer` state, including in-water,
  per tick. This is the highest-fidelity source, but also the most engineering effort.
- I found no official automation or test API for the client.

## 4. How anticheat devs derived and validated physics

- **oomph/bedsim** ([bedsim](https://github.com/oomph-ac/bedsim), MIT, Go):
  - Ported from ethaniccc's oomph; liquid physics come from oomph#145 (NopeNotDark).
  - Its tests (`parity_regressions_test.go`, `liquid_hardening_test.go`, and others) are
    **hand-written synthetic cases against a mock world**, not recorded traces. No testdata/fixtures
    are checked in.
  - Real validation is live: the sim result is compared with client PAI each tick, and the
    `movement_sim` debug mode prints the diffs.
- **Boar** ([opencollab-incubator/Boar](https://github.com/opencollab-incubator/Boar)):
  - A Geyser extension that implements server-auth-with-rewind, "designed based off the vanilla
    movement code". It admits "a lot of movement differences are not implemented".
  - It has no trace dataset.
- **GrimAC**: exempts Geyser/Bedrock players (Floodgate); it is Java physics only. Not useful for
  Bedrock.
- **Mojang's own spec**
  ([PlayerMovementOverview.md](https://github.com/Mojang/bedrock-protocol-docs/blob/main/additional_docs/PlayerMovementOverview.md)):
  - The client predicts, then sends PAI with its tick id.
  - The server simulates only when a PAI arrives.
  - A `CorrectPlayerMovePrediction` (161) carries the tick id, and the client rewinds and replays.
  - `SetActorData` / `UpdateAttributes` / `MovePlayer` can also carry tick ids.
  - This defines the alignment rules for our trace format.
- **Public trace datasets:** none found. Treat this as a gap we fill ourselves.
- Uncertain: the exact semantics of PAI `Delta` (post-friction velocity vs position delta) and when
  `on_ground` is derived relative to the move. Check against oomph's use of `Delta`, and against CPMP
  `OnGround`, before labelling fields.

## 5. Legal

- [Minecraft EULA](https://www.minecraft.net/en-us/eula):
  - "Mods are okay to distribute; hacked versions or Modded Versions of the game client or server
    software are not okay to distribute."
  - "Do not distribute or make commercial use of anything we've made without our permission."
- The EULA text has no explicit reverse-engineering clause (grepped 2026-09-30). The Microsoft
  Services Agreement also applies, and its standard software terms restrict decompiling except where
  law permits. Unverified wording.
- Practical reading: private use of LeviLamina/Endstone/proxies is tolerated ecosystem practice.
  - Do not redistribute BDS binaries, patched binaries or extracted code/tables.
  - Keep traces as our own measured data (positions and flags). That is fine to commit.
  - Do not paste decompiled Mojang code into the Rust port.
- Mojang does not support modded BDS (per [wiki](https://minecraft.wiki/w/Bedrock_Dedicated_Server)).
- Not legal advice.

## Proposed trace format (one JSONL row per client tick)
`{tick, input:{move_vec, raw_move_vec, flags[], yaw, pitch}, client:{pos, delta, hcoll, vcoll}, server_corr?:{pos, delta, on_ground}}`
Add `in_water` and `travel_type` columns only if we build the LeviLamina hook in step 3.
