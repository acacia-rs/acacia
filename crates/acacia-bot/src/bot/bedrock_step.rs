use acacia_client::proto::RawPacket;
use acacia_client::Event;

use super::Step;
use crate::events::BotEvent;
use crate::lag::LagLine;
use crate::movement::Movement;
use crate::state::PlayerState;
use crate::trace;
use crate::world::WorldTracker;
use crate::Bot;

impl Bot {
    /// [`Bot::step`] on Bedrock.
    pub(super) async fn bedrock_step(&mut self) -> Option<Step> {
        let b = self.backend.bedrock_mut()?;
        if let Some(world) = &mut b.world {
            for request in world.outgoing.drain(..) {
                b.client.send(&request);
            }
            let rejected = world.rejected.drain(..).filter(|_| b.strict);
            self.out.extend(rejected.map(BotEvent::Violation));
        }
        if let Some(packet) = b.lag.as_mut().and_then(|l| l.pop_due(tokio::time::Instant::now())) {
            self.apply(&packet);
            return Some(Step::Packet(packet));
        }
        // A queued event with no tick due skips the select!, whose tick timer was registered and cleared per packet.
        if b.lag.is_none()
            && !self.ticker.due()
            && let Some(event) = b.client.try_recv()
        {
            return self.on_event(event);
        }
        let lagged = b.lag.as_ref().and_then(LagLine::next_at);
        tokio::select! {
            event = b.client.recv() => self.on_event(event?),
            // In an async block: select! builds a disabled branch's future too, and a bare sleep looks up the timer.
            _ = async { tokio::time::sleep_until(lagged.unwrap_or_else(tokio::time::Instant::now)).await }, if lagged.is_some() => Some(Step::Idle),
            Some(request) = self.inbox.rx.recv() => {
                self.on_immediate(request);
                Some(Step::Idle)
            }
            ticks = self.ticker.wait() => {
                self.run_ticks(ticks);
                Some(Step::Tick)
            }
        }
    }

    fn on_event(&mut self, event: Event) -> Option<Step> {
        match event {
            Event::Packet(packet) => {
                if let Some(lag) = self.backend.bedrock_mut().and_then(|b| b.lag.as_mut()) {
                    lag.push(tokio::time::Instant::now(), packet);
                    return Some(Step::Idle);
                }
                self.apply(&packet);
                Some(Step::Packet(packet))
            }
            Event::Violation(violation) => {
                self.out.push(BotEvent::Violation(violation));
                Some(Step::Idle)
            }
            Event::Disconnected(reason) => {
                self.closed = Some(reason.clone());
                Some(Step::Disconnected(reason))
            }
        }
    }

    fn apply(&mut self, packet: &RawPacket) {
        let Some(b) = self.backend.bedrock_mut() else { return };
        if let Some(r) = &mut b.recorder
            && (PlayerState::PACKETS.contains(&packet.id)
                || WorldTracker::PACKETS.contains(&packet.id)
                || Movement::PACKETS.contains(&packet.id)
                || trace::ANALYSIS_PACKETS.contains(&packet.id) && trace::about(packet, self.state.me().runtime_entity_id))
        {
            r.write(&trace::Event::Packet(packet.clone()));
        }
        let was_alive = self.state.player.alive;
        let mut result = self.state.apply(packet);
        let me = self.state.me();
        if let Some(world) = &mut b.world {
            result = result.and(world.apply(packet));
        }
        if let Some(movement) = &mut b.movement {
            result = result.and(movement.apply(packet, &me));
        }
        if let Err(e) = result {
            tracing::debug!(id = packet.id, error = %e, "tracker failed to decode packet");
        }
        self.on_container_close(packet);
        if let Some(b) = self.backend.bedrock_mut() {
            b.sync.apply(packet, self.state.player.runtime_entity_id);
            if let Some(subchunks) = &mut b.subchunks {
                subchunks.apply(packet);
            }
        }
        self.on_travel_packet(packet);
        self.auto_respawn(packet, was_alive);
        self.reflex_packet(packet);
        self.on_fetch_packet(packet);
        self.publish_entity_changes();
        self.events.on_packet(packet, &mut self.state, &mut self.out);
    }
}
