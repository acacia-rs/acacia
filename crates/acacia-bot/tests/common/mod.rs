use std::time::Duration;

use acacia_bot::swarm::SwarmEvent;
use tokio::sync::broadcast;
use tokio::time::timeout;

/// The first event matching `want`, skipping the rest.
pub async fn wait_for(events: &mut broadcast::Receiver<SwarmEvent>, want: impl Fn(&SwarmEvent) -> bool) -> SwarmEvent {
    timeout(Duration::from_secs(20), async {
        loop {
            let event = events.recv().await.expect("event stream open");
            if want(&event) {
                return event;
            }
        }
    })
    .await
    .expect("event within 20 s")
}
