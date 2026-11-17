use std::collections::HashMap;
use std::sync::Arc;

use acacia_client::proto::packets::{
    ClientCacheMissResponse, CorrectPlayerMovePrediction, CorrectPlayerMovePredictionPredictionType, MovePlayer, PlayerAuthInput, SetEntityData, SetEntityMotion,
};
use acacia_client::{BlobStore, MemoryBlobStore};
use acacia_client::proto::types::{InputData as F, MetadataFlags1, Vec3f};
use acacia_client::proto::Packet;
use acacia_physics::Vec3;

use super::blocks::nearby_blocks;
use super::report::{dist, CorrectionDiff, Report, TickDiff};
use super::ride::{Driven, Seat};
use super::{Event, Ride};
use crate::movement::{Controls, Movement, VehicleCorrection, EYE_HEIGHT};
use crate::state::GameState;
use crate::world::{PhysicsWorld, SharedWorlds, WorldTracker};

/// Ticks around a teleport or knockback in which a correction can be part of it.
const EVENT_WINDOW: u64 = 30;
/// Ticks around a teleport or knockback's arrival in which a recorded client may have applied it
/// differently (the proxy sees packets before the client does).
const ARRIVAL_WINDOW: u64 = 2;

/// Ticks a freeze reported late keeps the position off by more than the tolerance.
const FREEZE_CARRY: u64 = 2;

/// Replays a trace through [`Movement`] with the recorded controls, and a driven vehicle's seated inputs
/// through its simulation. With `resync`, the state is reset to the recorded position and velocity after
/// every diverging tick, so each divergence is reported once instead of cascading (use for traces of real
/// clients).
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
    let mut seat: Option<Seat> = None;
    let mut solids = Vec::new();
    let mut driven: HashMap<u64, Driven> = HashMap::new();
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
            Event::GlideBoost => movement.predict_glide_boost(),
            Event::Riptide(level) => {
                movement.predict_riptide(*level, true);
            }
            Event::Solids(boxes) => solids.clone_from(boxes),
            Event::Ride(ride) => {
                seat = Seat::start(ride);
                if let Ride::Leave { feet } = ride
                    && let (Some(view), Some(registry)) = (world.view(), world.registry())
                {
                    movement.dismount(last_tick, *feet, &PhysicsWorld { view, registry });
                }
            }
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
                    if c.prediction_type == CorrectPlayerMovePredictionPredictionType::Vehicle {
                        // Stored for the next seated input, as the bot takes it.
                        let _ = movement.apply(p, &state.me());
                        if let Some(vc) = movement.vehicle_correction {
                            report.corrections.push(vehicle_diff(&vc, driven.get(&vc.tick), mark.clone(), &world));
                        }
                        continue;
                    }
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
                    let deltas = (ours.get(&tick).map(|&(_, d)| d), vec3(&c.delta));
                    report.corrections.push(CorrectionDiff {
                        tick,
                        mark: mark.clone(),
                        ours: pos,
                        server,
                        delta_err: delta,
                        deltas,
                        sent,
                        explained,
                        blocks,
                        vehicle: false,
                    });
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
                if let Some(seat) = &mut seat {
                    last_tick = rec.tick;
                    // The dismount tick reports no vehicle; nothing is simulated on it.
                    if !rec.input_data.contains(&F::ClientPredictedVehicle) {
                        continue;
                    }
                    let correction = movement.vehicle_correction.take();
                    let (Some(view), Some(registry)) = (world.view(), world.registry()) else { continue };
                    let controls = controls_of(&rec, false, false);
                    let Some(d) = seat.step(rec.tick, &controls, correction.as_ref(), &PhysicsWorld { view, registry }) else { continue };
                    tracing::trace!(tick = rec.tick, pos = ?d.pos, delta = ?d.delta, yaw = d.yaw, "replayed vehicle");
                    driven.insert(rec.tick, d);
                    report.ticks += 1;
                    let delta_err = dist(d.delta, vec3(&rec.delta));
                    if dist(d.pos, recorded) > tolerance || delta_err > tolerance {
                        let blocks = nearby_blocks(&world, recorded);
                        report.diverged.push(TickDiff { tick: rec.tick, mark: mark.clone(), ours: d.pos, recorded, delta_err, blocks, near_event: false });
                    }
                    continue;
                }
                movement.align_tick(rec.tick);
                movement.set_abilities(&state.player);
                movement.controls = controls_of(&rec, movement.controls.glide, movement.flying());
                // Traces don't record armour; a glide start means an elytra was worn.
                movement.elytra |= rec.input_data.contains(&F::StartGliding);
                movement.recorded_want_down = Some(rec.input_data.contains(&F::WantDown));
                movement.set_solids(solids.clone());
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

/// A driven vehicle's correction against our vehicle after the input it names.
fn vehicle_diff(c: &VehicleCorrection, ours: Option<&Driven>, mark: Option<String>, world: &WorldTracker) -> CorrectionDiff {
    CorrectionDiff {
        tick: c.tick,
        mark,
        ours: ours.map(|d| d.pos),
        server: c.feet,
        delta_err: ours.map(|d| dist(d.delta, c.delta)),
        deltas: (ours.map(|d| d.delta), c.delta),
        sent: None,
        // No replayed tick: the state as boarded, before the ride's first input (riding/boat.rs `correct`).
        explained: ours.is_none_or(|d| d.wave_only(c)),
        blocks: nearby_blocks(world, c.feet),
        vehicle: true,
    }
}

/// The held controls a `PlayerAuthInput` reports. Gliding shows only as Start/StopGliding edges, so it carries
/// over from `gliding`, the previous input's state. `flying` is the simulation's: the recorded jump presses
/// toggle the flight themselves.
fn controls_of(p: &PlayerAuthInput, gliding: bool, flying: bool) -> Controls {
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
        fly: flying,
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
