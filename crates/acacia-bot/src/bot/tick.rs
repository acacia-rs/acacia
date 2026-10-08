use acacia_client::proto::packets::PlayerAuthInput;
use acacia_client::proto::types::{Action, InputData};

use crate::spawn::{self, SpawnPacket};
use crate::trace;
use crate::world::PhysicsWorld;
use crate::Bot;

/// Start movement after this long with the spawn chunk loaded even without support below (5 s).
const SPAWN_WAIT_TICKS: u32 = 100;

impl Bot {
    /// One client tick: scheduled actions, then idle input or the physics simulation.
    pub(super) fn on_tick(&mut self) {
        self.tick_respawn();
        self.tick_reflexes();
        self.tick_survival();
        if let Some(sync) = self.sync.tick(&self.state.player) {
            self.client.send(&sync);
        }
        if let Some(subchunks) = &mut self.subchunks {
            let feet = &self.state.player.position;
            for request in subchunks.tick(feet.x, feet.z) {
                self.client.send(&request);
            }
            if let Some(world) = &mut self.world {
                world.follow(feet.x, feet.z);
            }
        }
        if self.idle.is_some() {
            self.idle_tick();
            return;
        }
        if self.has_physics() && self.state.player.alive && (self.ride_physics_tick() || self.bed_physics_tick()) {
            return;
        }
        self.tick_mining();
        self.physics_tick();
    }

    /// Adds the one-tick flags actions queued (gliding, item use) to this tick's input, skipping any it
    /// already carries: BDS drops the player for a `PlayerAuthInput` listing a flag twice.
    pub(crate) fn add_queued_flags(&mut self, input: &mut PlayerAuthInput) {
        for flag in self.queued_flags.drain(..) {
            if !input.input_data.contains(&flag) {
                input.input_data.push(flag);
            }
        }
    }

    fn idle_tick(&mut self) {
        let Some((idle, sequence)) = self.idle.as_mut() else { return };
        let step = sequence.tick();
        let input = if step.input { idle.tick(&self.state.player, sequence.screen_mode()) } else { None };
        let me = self.state.player.runtime_entity_id;
        step.before.into_iter().for_each(|p| spawn::send(&self.client, me, p));
        if let Some(mut input) = input {
            self.seat_idle_input(&mut input);
            self.add_queued_flags(&mut input);
            self.client.send(&input);
        }
        if let Some(pause) = step.stall {
            self.ticker.stall(pause);
        }
        if step.after.contains(&SpawnPacket::Settled) {
            self.sync.spawned();
        }
        step.after.into_iter().for_each(|p| spawn::send(&self.client, me, p));
    }

    fn physics_tick(&mut self) {
        let elytra = self.wears_elytra();
        let (Some(world), Some(movement)) = (&self.world, &mut self.movement) else { return };
        movement.elytra = elytra;
        let (Some(view), Some(registry)) = (world.view(), world.registry()) else { return };
        if !movement.is_started() {
            let p = &self.state.player;
            let feet = [p.position.x, p.position.y, p.position.z];
            // Geyser's StartGame position is a placeholder until its first teleport, so wait for ground or
            // liquid near the feet as well as the chunk (or give up waiting when spawned mid-air), then
            // leave the loading screen like vanilla.
            if !crate::world::is_chunk_loaded(view, feet) {
                return;
            }
            movement.spawn_wait += 1;
            if !crate::world::has_support_below(view, registry, feet) && movement.spawn_wait < SPAWN_WAIT_TICKS {
                return;
            }
            // TODO: physics bots skip vanilla's spawn timing (crate::spawn); idle bots follow it.
            for packet in [SpawnPacket::LoadingScreenStart, SpawnPacket::LoadingScreenEnd, SpawnPacket::Initialized] {
                spawn::send(&self.client, p.runtime_entity_id, packet);
            }
            movement.start(feet, p.yaw, p.pitch);
            self.sync.spawned();
            if let Some(r) = &mut self.recorder {
                r.write(&trace::Event::Start { feet, yaw: p.yaw, pitch: p.pitch });
            }
            return;
        }
        // Dead players don't move; simulating on would sink or fall away from the server's position.
        if !self.state.player.alive {
            return;
        }
        let worn = crate::movement::equipment::worn(&self.state.inventory, &self.state.items);
        if movement.set_equipment(worn)
            && let Some(r) = &mut self.recorder
        {
            r.write(&trace::Event::Equipment(worn));
        }
        movement.set_abilities(&self.state.player);
        let pending = std::mem::take(&mut movement.pending_actions);
        if let Some(mut input) = movement.tick(&PhysicsWorld { view, registry }) {
            if let Some(r) = &mut self.recorder {
                r.input(&input);
            }
            crate::movement::attach_actions(&mut input, pending);
            self.add_queued_flags(&mut input);
            // BDS grants the input's StartFlying only after this action (its handler checks the may-fly rights).
            if input.input_data.contains(&InputData::StartFlying) {
                let me = self.state.player.runtime_entity_id;
                self.client.send(&crate::sleep::player_action(me, Action::StartFlying));
            }
            self.client.send(&input);
        }
    }
}
