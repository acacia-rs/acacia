use std::collections::HashMap;
use std::sync::Arc;

use acacia_client::proto::packets::{ClientCacheMissResponse, CorrectPlayerMovePrediction, MovePlayer, PlayerAuthInput, SetEntityData, SetEntityMotion};
use acacia_client::{BlobStore, MemoryBlobStore};
use acacia_client::proto::types::{InputData as F, MetadataFlags1, Vec3f};
use acacia_client::proto::Packet;
use acacia_physics::Vec3;

use super::Event;
use crate::movement::{Controls, Movement, EYE_HEIGHT};
use crate::state::GameState;
use crate::world::{PhysicsWorld, SharedWorlds, WorldTracker};

/// Ticks around a teleport or knockback in which a correction can be part of it.
const EVENT_WINDOW: u64 = 30;
/// Ticks around a teleport or knockback's arrival in which a recorded client may have applied it
/// differently (the proxy sees packets before the client does).
const ARRIVAL_WINDOW: u64 = 2;

/// A tick whose replayed output differs from the recorded `PlayerAuthInput`.
#[derive(Debug, Clone)]
pub struct TickDiff {
    pub tick: u64,
    pub mark: Option<String>,
    /// Eye positions.
    pub ours: Vec3,
    pub recorded: Vec3,
    pub delta_err: f32,
    /// Blocks around the recorded position, as in [`CorrectionDiff::blocks`].
    pub blocks: String,
    /// Within [`ARRIVAL_WINDOW`] ticks of a teleport or knockback arriving or taking effect.
    pub near_event: bool,
}

/// A server correction compared with the replayed position at the tick it names.
#[derive(Debug, Clone)]
pub struct CorrectionDiff {
    pub tick: u64,
    pub mark: Option<String>,
    /// Eye positions; `ours` is `None` when the tick was not replayed.
    pub ours: Option<Vec3>,
    pub server: Vec3,
    /// Distance between our velocity and the server's at that tick.
    pub delta_err: Option<f32>,
    /// The eye position and velocity the recorded client sent for that tick (a real client's own physics).
    pub sent: Option<(Vec3, Vec3)>,
    /// Explained by what the client had not been sent yet rather than a disagreement: a correction to a
    /// teleport's target, or one for a tick the server moved us on before we heard of it, froze us by an
    /// amount it reported later, or simulated in a chunk we had not received.
    pub explained: bool,
    /// Non-air blocks around the server's position (both layers), for triage.
    pub blocks: String,
}

impl CorrectionDiff {
    pub fn error(&self) -> Option<f32> {
        self.ours.map(|o| dist(o, self.server))
    }

    /// A disagreement in position or velocity beyond `tolerance` (a correction for a tick we did not
    /// simulate counts).
    pub fn mismatch(&self, tolerance: f32) -> bool {
        !self.explained && (self.error().is_none_or(|e| e > tolerance) || self.delta_err.is_some_and(|e| e > tolerance))
    }
}

/// Server corrections within this of our position are periodic resyncs, not mismatches.
pub const CORRECTION_TOLERANCE: f32 = 0.001;
/// Farther than any tick of movement: BDS applied a `/tp` it has not sent us yet (it can lag many ticks).
const TELEPORT_LAG_DISTANCE: f32 = 4.0;
/// Ticks a freeze reported late keeps the position off by more than the tolerance.
const FREEZE_CARRY: u64 = 2;

#[derive(Debug, Default)]
pub struct Report {
    pub ticks: usize,
    /// Ticks off the recording by more than the tolerance.
    pub diverged: Vec<TickDiff>,
    pub corrections: Vec<CorrectionDiff>,
}

impl Report {
    /// Corrections we disagree with beyond `tolerance`, and those skipped as teleport lags.
    pub fn correction_mismatches(&self, tolerance: f32) -> (Vec<&CorrectionDiff>, Vec<&CorrectionDiff>) {
        let (lagged, mismatches) = self
            .corrections
            .iter()
            .filter(|c| c.mismatch(tolerance))
            .partition(|c| c.error().is_some_and(|e| e > TELEPORT_LAG_DISTANCE));
        (mismatches, lagged)
    }
}

/// Replays a trace through [`Movement`] with the recorded controls. With `resync`, the state is reset
/// to the recorded position and velocity after every diverging tick, so each divergence is reported
/// once instead of cascading (use for traces of real clients).
pub fn replay(events: &[Event], tolerance: f32, resync: bool) -> Report {
    let mut state = GameState::default();
    let mut world = WorldTracker::new("replay".into(), SharedWorlds::new());
    // The recording client held every blob it was sent; later sections name them by id only.
    let blobs = Arc::new(MemoryBlobStore::with_payloads());
    world.set_blob_store(blobs.clone());
    let mut movement = Movement::new();
    // Our eye position and velocity per input tick.
    let mut ours: HashMap<u64, (Vec3, Vec3)> = HashMap::new();
    let mut sent: HashMap<u64, (Vec3, Vec3)> = HashMap::new();
    // The freeze each input tick was simulated with; `None` in a chunk not received yet.
    let mut frozen: HashMap<u64, Option<f32>> = HashMap::new();
    let mut mark: Option<String> = None;
    // (input tick it takes effect on, teleport target, our tick when it arrived)
    let mut moves: Vec<(u64, Option<Vec3>, u64)> = Vec::new();
    let mut last_tick = 0;
    let mut report = Report::default();
    for event in events {
        match event {
            Event::Mark(label) => mark = Some(label.clone()),
            Event::Equipment(worn) => {
                movement.set_equipment(*worn);
            }
            Event::Start { feet, yaw, pitch } => movement.start(*feet, *yaw, *pitch),
            Event::Packet(p) => {
                if p.id == ClientCacheMissResponse::ID
                    && let Ok(r) = p.decode::<ClientCacheMissResponse>()
                {
                    for blob in &r.blobs {
                        blobs.insert(blob.hash, &blob.payload);
                    }
                }
                let _ = state.apply(p);
                let _ = world.apply(p);
                world.outgoing.clear();
                if p.id == MovePlayer::ID
                    && let Ok(m) = p.decode::<MovePlayer>()
                    && u64::from(m.runtime_id) == state.me().runtime_entity_id
                {
                    moves.push((m.tick + 1, Some(vec3(&m.position)), last_tick));
                }
                if p.id == SetEntityData::ID
                    && let Ok(d) = p.decode::<SetEntityData>()
                    && d.runtime_entity_id == state.me().runtime_entity_id
                    && let Some(f) = super::actor_flags(&d)
                {
                    let (sprinting, swimming) = (f.0 & MetadataFlags1::SPRINTING.0 != 0, f.0 & MetadataFlags1::SWIMMING.0 != 0);
                    tracing::trace!(tick = last_tick, server_tick = d.tick, sprinting, swimming, "server actor flags");
                }
                if p.id == SetEntityMotion::ID
                    && let Ok(m) = p.decode::<SetEntityMotion>()
                    && m.runtime_entity_id == state.me().runtime_entity_id
                {
                    moves.push((m.tick + 1, None, last_tick));
                }
                if p.id == CorrectPlayerMovePrediction::ID
                    && let Ok(c) = p.decode::<CorrectPlayerMovePrediction>()
                {
                    let server = vec3(&c.position);
                    // Geyser stamps no tick: compare with the latest tick instead.
                    let tick = if c.tick == 0 { last_tick } else { c.tick };
                    let blocks = nearby_blocks(&world, feet(server));
                    let (pos, delta) = ours.get(&tick).map_or((None, None), |&(p, d)| (Some(p), Some(dist(d, vec3(&c.delta)))));
                    let sent = sent.get(&tick).copied();
                    tracing::trace!(tick, ours = ?ours.get(&tick).map(|&(_, d)| d), server = ?vec3(&c.delta), "correction delta");
                    // A late freeze still shows in the position two ticks on.
                    let explained = (tick.saturating_sub(FREEZE_CARRY)..=tick).any(|t| match frozen.get(&t) {
                        Some(Some(used)) => movement.freeze_at(t).1.is_some_and(|f| (f - used).abs() > 1e-6),
                        Some(None) => true,
                        None => false,
                    });
                    report.corrections.push(CorrectionDiff { tick, mark: mark.clone(), ours: pos, server, delta_err: delta, sent, explained, blocks });
                }
                let _ = movement.apply(p, &state.me());
            }
            Event::Input(body) => {
                let Ok(rec) = PlayerAuthInput::decode(&mut &body[..]) else { continue };
                let recorded = vec3(&rec.position);
                tracing::trace!(tick = rec.tick, pos = ?recorded, delta = ?vec3(&rec.delta), flags = ?rec.input_data, "sent input");
                if !movement.is_started() {
                    // A real client's trace: start from its first reported state.
                    movement.start(feet(recorded), rec.yaw, rec.pitch);
                    movement.resync(rec.tick, feet(recorded), vec3(&rec.delta));
                    continue;
                }
                movement.align_tick(rec.tick);
                movement.controls = controls_of(&rec, movement.controls.glide);
                // Traces don't record armour; a glide start means an elytra was worn.
                movement.elytra |= rec.input_data.contains(&F::StartGliding);
                movement.recorded_want_down = Some(rec.input_data.contains(&F::WantDown));
                let (Some(view), Some(registry)) = (world.view(), world.registry()) else { continue };
                let Some(out) = movement.tick(&PhysicsWorld { view, registry }) else { continue };
                let pos = vec3(&out.position);
                ours.insert(out.tick, (pos, vec3(&out.delta)));
                sent.insert(out.tick, (recorded, vec3(&rec.delta)));
                let received = view.chunk((pos[0].floor() as i32) >> 4, (pos[2].floor() as i32) >> 4).is_some();
                frozen.insert(out.tick, received.then(|| movement.freeze_at(out.tick).0));
                last_tick = out.tick;
                report.ticks += 1;
                let delta_err = dist(vec3(&out.delta), vec3(&rec.delta));
                if dist(pos, recorded) > tolerance || delta_err > tolerance {
                    let blocks = nearby_blocks(&world, feet(recorded));
                    report.diverged.push(TickDiff { tick: out.tick, mark: mark.clone(), ours: pos, recorded, delta_err, blocks, near_event: false });
                    if resync {
                        movement.resync(out.tick, feet(recorded), vec3(&rec.delta));
                    }
                }
            }
        }
    }
    for d in &mut report.diverged {
        d.near_event = moves.iter().any(|&(effect, _, arrived)| {
            d.tick.abs_diff(effect) <= ARRIVAL_WINDOW || d.tick.abs_diff(arrived) <= ARRIVAL_WINDOW
        });
    }
    for c in &mut report.corrections {
        c.explained |= moves.iter().any(|&(effect, target, arrived)| {
            let at_target = target.is_some_and(|t| dist(t, c.server) < 1e-3);
            effect.abs_diff(c.tick) <= EVENT_WINDOW && (at_target || (effect <= c.tick && arrived >= c.tick))
        });
    }
    report
}

/// Distinct non-air block names in the player's box (widened by 0.1) and the layer below its feet.
fn nearby_blocks(world: &WorldTracker, [x, y, z]: Vec3) -> String {
    use acacia_world::BlockAccess;
    let (Some(view), Some(registry)) = (world.view(), world.registry()) else { return String::new() };
    let span = |lo: f32, hi: f32| lo.floor() as i32..=hi.floor() as i32;
    let mut names: Vec<&str> = Vec::new();
    for bx in span(x - 0.4, x + 0.4) {
        for by in span(y - 1.0, y + 1.9) {
            for bz in span(z - 0.4, z + 0.4) {
                for id in [view.block(bx, by, bz), view.liquid(bx, by, bz)] {
                    if let Some(s) = registry.get(id).filter(|s| !s.is_air()) {
                        let name = s.name.trim_start_matches("minecraft:");
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
            }
        }
    }
    // Liquid levels (Bedrock liquid_depth) in the 5x5 around the feet, at and below their layer.
    let (fx, fy, fz) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    let mut liquids = Vec::new();
    for by in [fy, fy - 1] {
        for bz in fz - 2..=fz + 2 {
            for bx in fx - 2..=fx + 2 {
                let state = [(view.block(bx, by, bz), ""), (view.liquid(bx, by, bz), "L")]
                    .into_iter()
                    .filter_map(|(id, layer)| registry.get(id).map(|s| (s, layer)))
                    .find(|(s, _)| s.is_water());
                // `L` marks water in the second (liquid) layer.
                if let Some((s, layer)) = state {
                    liquids.push(format!("{},{},{}:{}{layer}", bx - fx, by - fy, bz - fz, s.liquid_depth));
                }
            }
        }
    }
    // Blocks other than stone in the 3x3 below the feet (surface effects) and at the feet (thin blocks).
    let layer = |by: i32| {
        let mut found = Vec::new();
        for bz in fz - 1..=fz + 1 {
            for bx in fx - 1..=fx + 1 {
                if let Some(s) = registry.get(view.block(bx, by, bz)).filter(|s| !s.is_air() && !s.name.ends_with(":stone")) {
                    found.push(format!("{},{}:{}", bx - fx, bz - fz, s.name.trim_start_matches("minecraft:")));
                }
            }
        }
        found
    };
    let mut out = names.join(",");
    for (label, found) in [("floor", layer(fy - 1)), ("feet", layer(fy))] {
        if !found.is_empty() {
            out += &format!(" {label} {}", found.join(" "));
        }
    }
    if !liquids.is_empty() {
        out += &format!(" water {}", liquids.join(" "));
    }
    // Collision boxes of non-stone blocks in the 3x3 around the feet, feet to head layer.
    if std::env::var_os("BTRC_BOXES").is_some() {
        for by in fy..=fy + 2 {
            for bz in fz - 1..=fz + 1 {
                for bx in fx - 1..=fx + 1 {
                    let Some(s) = registry.get(view.block(bx, by, bz)).filter(|s| !s.boxes.is_empty() && !s.name.ends_with(":stone")) else { continue };
                    let boxes: Vec<_> = s.boxes.iter().map(|b| (b.min, b.max)).collect();
                    out += &format!("\n  [{bx},{by},{bz}] {} {boxes:?}", s.name);
                }
            }
        }
    }
    if std::env::var_os("BTRC_MAP").is_some() {
        out += &block_map(view, registry, [fx, fy, fz]);
    }
    out
}

/// Layers feet-1..=feet+2 of the 5x5 around the feet (x across, z down): `.` air, `#` full block, `~` water
/// (with its depth), else the first letter of the name.
fn block_map(view: &impl acacia_world::BlockAccess, registry: &acacia_world::BlockRegistry, [fx, fy, fz]: [i32; 3]) -> String {
    let mut out = String::new();
    for by in fy - 1..=fy + 2 {
        out += &format!("\n  y{:+}", by - fy);
        for bz in fz - 2..=fz + 2 {
            out += " ";
            for bx in fx - 2..=fx + 2 {
                let solid = registry.get(view.block(bx, by, bz)).filter(|s| !s.is_air());
                let water = [view.block(bx, by, bz), view.liquid(bx, by, bz)].into_iter().filter_map(|id| registry.get(id)).find(|s| s.is_water());
                out += &match (solid, water) {
                    (Some(s), _) if !s.is_water() => {
                        let n = s.name.trim_start_matches("minecraft:");
                        if n == "stone" { "#".into() } else { n[..1].to_string() }
                    }
                    (_, Some(w)) => format!("{}", w.liquid_depth % 10),
                    _ => ".".into(),
                };
            }
        }
    }
    out
}

/// The held controls a `PlayerAuthInput` reports. Gliding shows only as Start/StopGliding edges, so it
/// carries over from `gliding`, the previous input's state.
fn controls_of(p: &PlayerAuthInput, gliding: bool) -> Controls {
    let has = |f: F| p.input_data.contains(&f);
    let axis = |pos: bool, neg: bool| f32::from(u8::from(pos)) - f32::from(u8::from(neg));
    Controls {
        forward: axis(has(F::Up), has(F::Down)),
        strafe: axis(has(F::Left), has(F::Right)),
        jump: has(F::JumpDown),
        sneak: has(F::SneakDown),
        // BDS reads `Sprinting`, not `SprintDown`, as the sprint intent: a bot may send its sprint state there.
        sprint: has(F::Sprinting),
        glide: (gliding || has(F::StartGliding)) && !has(F::StopGliding),
        yaw: p.yaw,
        pitch: p.pitch,
    }
}

fn vec3(v: &Vec3f) -> Vec3 {
    [v.x, v.y, v.z]
}

fn feet([x, y, z]: Vec3) -> Vec3 {
    [x, y - EYE_HEIGHT, z]
}

fn dist(a: Vec3, b: Vec3) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
