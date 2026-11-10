//! A goto tick on Java: the shared pilot over the Java chunks, with Java state ids as the planner's native ids.

use std::sync::Arc;

use acacia_java_client::Chunks;
use acacia_world::BlockRegistry;
use tokio::sync::oneshot;

use super::astar::SearchOpts;
use super::goto::NavStatus;
use super::job::{Answer, SearchJob};
use super::pilot::Pilot;
use super::terrain::{Blocks, Terrain};
use super::hazard::{self, Hazards};
use crate::bot::{Backend, Java};
use crate::movement::{JavaSim, Live, Sim};
use crate::state::GameState;
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

/// The steering half of [`super::goto::step`] on Java, with the threats [`perceive`]d from the mirrored entities.
/// Striking back is still Bedrock's.
pub(super) fn drive(pilot: &mut Pilot, bot: &mut Bot, mut opts: SearchOpts) -> Result<NavStatus, ActionError> {
    let Backend::Java(java) = &mut bot.backend else { return Ok(NavStatus::Searching) };
    let chunks = java.client.world.chunks().clone();
    opts.hazards = perceive(pilot, &bot.state, java.as_ref(), &chunks);
    let terrain = Terrain::new(&chunks, BlockRegistry::java()).with_hazards(opts.hazards.clone());
    pilot.drive(&terrain, java.as_mut(), opts, &mut |job| spawn(job, chunks.clone()))
}

/// The pilot's field this tick from the entities around the bot, as Bedrock's `goto::perceive`.
fn perceive(pilot: &mut Pilot, state: &GameState, java: &Java, chunks: &Chunks) -> Option<Arc<Hazards>> {
    let body = java.base()?.body();
    let threats = hazard::Threats::near(&state.entities, body.pos, hazard::FIELD_RADIUS);
    let blocks = Terrain::new(chunks, BlockRegistry::java());
    let sees = |a, b| hazard::clear_line(&blocks, a, b);
    let eye = [body.pos[0], body.pos[1] + JavaSim::envelope().eye_height, body.pos[2]];
    pilot.perceive(threats, &hazard::View { eye, yaw: body.yaw, health: state.player.health, sees: &sees })
}
