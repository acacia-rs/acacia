//! Crafting on Java: the ingredients go into the grid, then the result is shift-clicked, which crafts until the grid
//! runs out. Java servers send no recipes a bot can plan with, so the recipes are Java's vanilla ones
//! ([`crate::state::Recipes::java`]). A craft matching an unlocked book recipe is laid out by the server
//! ([`super::java_book`]), any other by hand ([`super::grid`]). The own grid needs the inventory open throughout:
//! closing it hands the grid back.

use std::time::Duration;

use acacia_client::proto::types::WindowType;
use acacia_physics::BlockPos;
use acacia_world::neutral::Item;

use super::craft::{grid_cells, pick_recipe};
use super::grid::{grid_batch, placement_clicks};
use crate::bot::Backend;
use crate::state::GameState;
use crate::{ActionError, Bot};

/// How long the server has to answer the result click with the grid emptied.
pub(super) const RESULT_TIMEOUT: Duration = Duration::from_secs(3);

impl Bot {
    /// [`Bot::craft_with`] on Java, whatever the mode.
    pub(super) async fn java_craft(&mut self, item: Item, count: u32, table: Option<BlockPos>) -> Result<u32, ActionError> {
        match table {
            Some(pos) => self.open_station_and_look(pos, &[WindowType::Workbench]).await?,
            None => self.hold_java_inventory(true),
        }
        let result = self.java_craft_loop(item, count, table.is_some()).await;
        let back = self.take_back(&grid_cells(table.is_some())).await;
        let closed = match table {
            Some(_) => self.close_station().await,
            None => {
                self.hold_java_inventory(false);
                self.java_close_inventory().await
            }
        };
        let made = result?;
        back.and(closed).map(|()| made)
    }

    async fn java_craft_loop(&mut self, item: Item, count: u32, table: bool) -> Result<u32, ActionError> {
        let cells = grid_cells(table);
        let mut made = 0;
        while made < count {
            let batch = pick_recipe(&self.state, item, table).and_then(|r| {
                let per_craft = r.outputs.first().map_or(1, |o| i32::from(o.count.max(1)));
                grid_batch(&self.state, r, table, count - made).map(|b| (per_craft, b))
            });
            let (per_craft, (times, placements)) = match batch {
                Ok(batch) => batch,
                Err(ActionError::NotPossible(_)) if made > 0 => break,
                Err(e) => return Err(e),
            };
            if self.java_place_from_book((i32::from(item.java_id()), per_craft), &placements, times).await? == 0 {
                for op in placement_clicks(&self.state, &placements) {
                    self.click_pause().await?;
                    self.item_stack_request(&[op]).await?;
                }
            }
            self.click_pause().await?;
            let before = held(&self.state, item);
            self.java_take_result(0).await?;
            let emptied = |bot: &Bot, _: &_| cells.iter().all(|c| c.stack(&bot.state).is_none_or(|s| s.is_empty())).then_some(());
            self.wait_until(RESULT_TIMEOUT, emptied).await?;
            let output = held(&self.state, item).saturating_sub(before);
            if output == 0 {
                return Err(ActionError::Rejected(format!("the server crafted no {item}")));
            }
            made += output;
        }
        Ok(made)
    }

    fn hold_java_inventory(&mut self, held: bool) {
        if let Backend::Java(java) = &mut self.backend {
            java.inventory_held = held;
        }
    }

    /// Closes the own inventory, sent with the next tick.
    async fn java_close_inventory(&mut self) -> Result<(), ActionError> {
        let Backend::Java(java) = &mut self.backend else { return Ok(()) };
        java.client.close_screen().map_err(|_| ActionError::Disconnected)?;
        self.next_tick(|_, _| false).await.map(|_| ())
    }
}

/// How many of `item` the main inventory holds.
pub(super) fn held(state: &GameState, item: Item) -> u32 {
    state.inventory.main.iter().filter(|s| !s.is_empty() && state.item(s) == Some(item)).map(|s| u32::from(s.count)).sum()
}
