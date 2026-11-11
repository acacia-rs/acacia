use acacia_client::proto::packets::PlayerAuthInput;
use acacia_client::proto::types::{Action, InputData};

use crate::spawn::{self, SpawnPacket};
use crate::trace;
use crate::world::PhysicsWorld;
use crate::Bot;

/// Start movement after this long with the spawn chunk loaded even without support below (5 s).
const SPAWN_WAIT_TICKS: u32 = 100;
/// Leave the loading screen after this long even with no terrain yet (2.5 s, vanilla's loading end after PlayerSpawn).
const TERRAINLESS_SPAWN_TICKS: u32 = 50;
/// Solid entities this near the feet (blocks) take part in the tick: more than a tick's travel.
const SOLID_REACH: f32 = 4.0;

impl Bot {
    /// The client ticks that came due, then the snapshot, entity moves and a [`crate::BotEvent::Tick`] for a sink
    /// (see [`crate::snapshot`], [`crate::events`]).
    pub(super) fn run_ticks(&mut self, ticks: u32) {
        for _ in 0..ticks {
            match self.backend {
                super::Backend::Bedrock(_) => self.on_tick(),
                #[cfg(feature = "java")]
                super::Backend::Java(_) => self.java_tick(),
            }
        }
        self.publish_snapshot(ticks);
        self.events.on_ticks(&self.state, &mut self.out);
        if self.out.is_sink() && self.events.wants(crate::Events::TICKS) {
            self.out.push(crate::BotEvent::Tick);
        }
    }

    /// One Bedrock client tick: scheduled actions, then idle input or the physics simulation.
    pub(super) fn on_tick(&mut self) {
        self.governor.tick(self.backend.movement().and_then(|m| m.setback_age()));
        self.state.entities.advance();
        self.tick_respawn();
        self.tick_reflexes();
        self.tick_survival();
        let Some(b) = self.backend.bedrock_mut() else { return };
        if let Some(sync) = b.sync.tick(&self.state.player) {
            b.client.send(&sync);
        }
        if let Some(subchunks) = &mut b.subchunks {
            let feet = &self.state.player.position;
            for request in subchunks.tick(feet.x, feet.z) {
                b.client.send(&request);
            }
            if let Some(world) = &mut b.world {
                world.follow(feet.x, feet.z);
            }
        }
        if b.idle.is_some() {
            self.idle_tick();
            return;
        }
        if self.has_physics() && self.state.player.alive && (self.ride_physics_tick() || self.bed_physics_tick()) {
            return;
        }
        self.tick_mining();
        self.physics_tick();
    }

    /// Adds `flag` to the next `PlayerAuthInput` (Bedrock only).
    pub(crate) fn queue_flag(&mut self, flag: InputData) {
        if let Some(b) = self.backend.bedrock_mut() {
            b.queued_flags.push(flag);
        }
    }

    /// Adds the one-tick flags actions queued (gliding, item use) to this tick's input, skipping any it
    /// already carries: BDS drops the player for a `PlayerAuthInput` listing a flag twice.
    pub(crate) fn add_queued_flags(&mut self, input: &mut PlayerAuthInput) {
        let Some(b) = self.backend.bedrock_mut() else { return };
        for flag in b.queued_flags.drain(..) {
            if !input.input_data.contains(&flag) {
                input.input_data.push(flag);
            }
        }
    }

    fn idle_tick(&mut self) {
        let Some(b) = self.backend.bedrock_mut() else { return };
        let Some((idle, sequence)) = b.idle.as_mut() else { return };
        let step = sequence.tick();
        let input = if step.input { idle.tick(&self.state.player, sequence.screen_mode()) } else { None };
        let me = self.state.player.runtime_entity_id;
        step.before.into_iter().for_each(|p| spawn::send(&b.client, me, p));
        if let Some(mut input) = input {
            self.seat_idle_input(&mut input);
            self.add_queued_flags(&mut input);
            self.send_bedrock(&input);
        }
        if let Some(pause) = step.stall {
            self.ticker.stall(pause);
        }
        let Some(b) = self.backend.bedrock_mut() else { return };
        if step.after.contains(&SpawnPacket::Settled) {
            b.sync.spawned();
        }
        step.after.into_iter().for_each(|p| spawn::send(&b.client, me, p));
        // The server is the source of an idle bot's position: nothing to wait for but its acknowledgement.
        self.travel_tick(true);
    }

    /// Starts the simulation at the server's position once the terrain there has arrived: at spawn, and
    /// again after each dimension change (dimension.rs).
    fn start_movement(&mut self) {
        let Some(b) = self.backend.bedrock_mut() else { return };
        let (Some(world), Some(movement)) = (&b.world, &mut b.movement) else { return };
        let (Some(view), Some(registry)) = (world.view(), world.registry()) else { return };
        let p = &self.state.player;
        let feet = [p.position.x, p.position.y, p.position.z];
        let travelled = b.travel.is_some();
        let mut placed = if travelled { super::dimension::terrain_arrived(view, feet) } else { crate::world::is_chunk_loaded(view, feet) };
        // Geyser's StartGame position is a placeholder until its first teleport, so wait for ground or
        // liquid near the feet as well as the chunk (or give up waiting when spawned mid-air), then
        // leave the loading screen like vanilla.
        if placed && !travelled {
            movement.spawn_wait += 1;
            placed = crate::world::has_support_below(view, registry, feet) || movement.spawn_wait >= SPAWN_WAIT_TICKS;
        }
        // gophertunnel proxies (Oomph) hold every backend packet, chunks included, until SetLocalPlayerAsInitialised.
        if !placed && !travelled && !b.spawned {
            b.spawn_ticks += 1;
            if b.spawn_ticks == TERRAINLESS_SPAWN_TICKS {
                send_spawn_packets(&b.client, &mut b.spawn_sent, p.runtime_entity_id);
            }
        }
        // After the spawn the look is the player's own; the server's is from before the change.
        let (yaw, pitch) = if b.spawned { (movement.controls.yaw, movement.controls.pitch) } else { (p.yaw, p.pitch) };
        if !self.travel_tick(placed) {
            return;
        }
        let Some(b) = self.backend.bedrock_mut() else { return };
        let Some(movement) = &mut b.movement else { return };
        movement.start(feet, yaw, pitch);
        if let Some(r) = &mut b.recorder {
            r.write(&trace::Event::Start { feet, yaw, pitch });
        }
        if std::mem::replace(&mut b.spawned, true) {
            return;
        }
        send_spawn_packets(&b.client, &mut b.spawn_sent, self.state.player.runtime_entity_id);
        b.sync.spawned();
    }

    fn physics_tick(&mut self) {
        let elytra = self.wears_elytra();
        if self.backend.movement().is_some_and(|m| !m.is_started()) {
            return self.start_movement();
        }
        let Some(b) = self.backend.bedrock_mut() else { return };
        let (Some(world), Some(movement)) = (&b.world, &mut b.movement) else { return };
        movement.elytra = elytra;
        let (Some(view), Some(registry)) = (world.view(), world.registry()) else { return };
        // Dead players don't move; simulating on would sink or fall away from the server's position.
        if !self.state.player.alive {
            return;
        }
        let worn = crate::movement::equipment::worn(&self.state.inventory, &self.state.items);
        if movement.set_equipment(worn)
            && let Some(r) = &mut b.recorder
        {
            r.write(&trace::Event::Equipment(worn));
        }
        movement.set_abilities(&self.state.player);
        if let Some(feet) = movement.position() {
            movement.set_solids(self.state.entities.solid_boxes(feet, SOLID_REACH));
        }
        let pending = std::mem::take(&mut movement.pending_actions);
        if let Some(mut input) = movement.tick(&PhysicsWorld { view, registry }) {
            if let Some(r) = &mut b.recorder {
                r.input(&input);
            }
            crate::movement::attach_actions(&mut input, pending);
            self.add_queued_flags(&mut input);
            // BDS grants the input's StartFlying only after this action (its handler checks the may-fly rights).
            if input.input_data.contains(&InputData::StartFlying) {
                let me = self.state.player.runtime_entity_id;
                self.send_bedrock(&crate::sleep::player_action(me, Action::StartFlying));
            }
            self.send_bedrock(&input);
        }
    }
}

/// Physics bots' loading screen and SetLocalPlayerAsInitialised, once.
// TODO: physics bots skip vanilla's spawn timing (crate::spawn); idle bots follow it.
fn send_spawn_packets(client: &acacia_client::Client, sent: &mut bool, me: u64) {
    if std::mem::replace(sent, true) {
        return;
    }
    for packet in [SpawnPacket::LoadingScreenStart, SpawnPacket::LoadingScreenEnd, SpawnPacket::Initialized] {
        spawn::send(client, me, packet);
    }
}
