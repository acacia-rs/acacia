//! [`Bot::fly_to`]: a [`FlightPilot`] driven live, with the elytra and rocket actions it orders.

use acacia_physics::BlockPos;

use super::pilot::{FlightPilot, Order};
use super::surface::{ChunkSurface, TopCache};
use super::FlyOpts;
use crate::pathfind::terrain::Terrain;
use crate::pathfind::{Goal, GotoOpts};
use crate::state::Inventory;
use crate::{ActionError, Bot};

const FIREWORK: &str = "minecraft:firework_rocket";
/// Landed further than this from the spot, the bot walks the rest.
const WALK_REST: f32 = 1.0;

impl Bot {
    /// Flies to `target` by elytra (worn, or put on from the inventory) and lands within `opts.land_radius` of it,
    /// on the top surface of the column (physics bots only). Rockets are used from the hotbar.
    pub async fn fly_to(&mut self, target: BlockPos, opts: FlyOpts) -> Result<(), ActionError> {
        if self.movement.as_ref().is_none_or(|m| !m.is_started()) {
            return Err(ActionError::NotPossible("flying needs a physics bot (BotConfig.physics)".into()));
        }
        self.equip_elytra().await?;
        if self.hotbar_rockets() > 0 {
            self.hold_fireworks().await?;
        }
        let mut pilot = FlightPilot::new(target, opts, crate::cadence::seed());
        let mut cache = TopCache::default();
        loop {
            let rockets = self.hotbar_rockets();
            let order = {
                let (Some(world), Some(movement)) = (self.world.as_ref(), self.movement.as_mut()) else { return Err(ActionError::Disconnected) };
                match (world.view(), world.registry(), movement.fork()) {
                    (Some(view), Some(registry), Some(fork)) => {
                        let terrain = Terrain::new(view, registry.as_ref());
                        let mut surface = ChunkSurface { view, registry: registry.as_ref(), cache: &mut cache };
                        pilot.tick(&fork, &terrain, &mut surface, rockets, &mut movement.controls)
                    }
                    _ => Order::Fly,
                }
            };
            match order {
                Order::Fly => {}
                // The pilot sees a glide that did not open and retries or gives up.
                Order::Open => match self.start_gliding().await {
                    Err(ActionError::Disconnected) => return Err(ActionError::Disconnected),
                    other => tracing::debug!(result = ?other, "elytra open"),
                },
                Order::Rocket => self.boost_with_firework().await?,
                Order::Landed => break,
                Order::Fail(why) => {
                    self.stop_pathing();
                    return Err(ActionError::NotPossible(why));
                }
            }
            self.next_tick(|_, _| false).await?;
        }
        self.stop_pathing();
        let (Some(spot), Some(feet)) = (pilot.spot(), self.movement.as_ref().and_then(|m| m.position())) else { return Ok(()) };
        if (spot[0] - feet[0]).hypot(spot[2] - feet[2]) > WALK_REST {
            let cell = spot.map(|v| v.floor() as i32);
            self.goto_with(Goal::Block(cell), GotoOpts::default()).await?;
        }
        Ok(())
    }

    fn hotbar_rockets(&self) -> u32 {
        let Some(id) = self.state.items.id(FIREWORK) else { return 0 };
        let hotbar = self.state.inventory.main.iter().take(usize::from(Inventory::HOTBAR_SLOTS));
        hotbar.filter(|s| s.network_id == id && !s.is_empty()).map(|s| u32::from(s.count)).sum()
    }
}
