//! A goto tick on Java: the shared pilot over the Java chunks, with Java state ids as the planner's native ids.

use acacia_java_client::Chunks;
use acacia_world::BlockRegistry;
use tokio::sync::oneshot;

use super::goto::{NavStatus, Worked};
use super::job::{Answer, SearchJob};
use super::pilot::Pilot;
use super::terrain::{Blocks, Terrain};
use crate::bot::Backend;
use crate::movement::JavaSim;
use crate::{ActionError, Bot};

const AIR: u32 = 0;

/// Java has no liquid layer: waterlogging is a block state, so the second id is always air.
impl Blocks for Chunks {
    fn ids(&self, pos: [i32; 3]) -> Option<(u32, u32)> {
        self.block(pos).map(|id| (id, AIR))
    }
}

fn spawn(job: SearchJob<JavaSim>, chunks: Chunks) -> Answer {
    let (tx, rx) = oneshot::channel();
    tokio::task::spawn_blocking(move || {
        let terrain = Terrain::new(&chunks, BlockRegistry::java()).with_hazards(job.opts.hazards.clone());
        let _ = tx.send(job.run(&terrain));
    });
    rx
}

/// [`super::goto::step`] on Java. Threat perception and path work (digging, bridging) are Bedrock-only so far.
// TODO: Java path work once dig and place go through the backend.
pub(super) fn step(pilot: &mut Pilot, bot: &mut Bot) -> (Result<NavStatus, ActionError>, Worked) {
    let opts = pilot.search_opts(None, bot.state.player.hunger);
    let Backend::Java(java) = &mut bot.backend else { return (Ok(NavStatus::Searching), Worked::default()) };
    let chunks = java.client.world.chunks().clone();
    let terrain = Terrain::new(&chunks, BlockRegistry::java());
    let status = pilot.drive(&terrain, java.as_mut(), opts, &mut |job| spawn(job, chunks.clone()));
    if status == Ok(NavStatus::Working) {
        return (pilot.work_failed("path work is not supported on Java Edition yet".into()).map(|()| NavStatus::Working), Worked::default());
    }
    (status, Worked::default())
}
