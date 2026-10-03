# formtest behaviour pack

Shows an action form, a modal, a custom form and a form to close to the first player who spawns, then
logs what BDS parsed (`FORMTEST ...` lines, needs `content-log-console-output-enabled=true`).

Setup (a second BDS so the main test server stays untouched):
1. Copy `.testserver/bds` without `worlds/` to `.testserver/bds-forms`; set `server-port=19160`,
   `server-portv6=19161`, `level-name=forms`.
2. Copy this folder to `bds-forms/development_behavior_packs/formtest`, and write
   `bds-forms/worlds/forms/world_behavior_packs.json`:
   `[{ "pack_id": "6f0c1d52-3a8e-4c1b-9f37-2b1f0f6e7a01", "version": [1, 0, 0] }]`
3. Start it, then `cargo run -p acacia-bot --example events -- 127.0.0.1:19160 FormBot 20`.

Expected log (verified 2026-10-02, BDS 1.26.52): `action selection=1`, `modal selection=0`,
`custom values=[null,"",true,5,1]`, `close canceled=true reason=UserClosed`.
