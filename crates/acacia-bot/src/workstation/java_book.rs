//! Java's recipe book: when the craft's grid matches an unlocked recipe, `PlaceRecipe` has the server lay it out, as
//! a player clicking the book does, instead of a click per stack.

use super::grid::Placement;
use super::java_craft::RESULT_TIMEOUT;
use crate::bot::Backend;
use crate::items::SlotRef;
use crate::{ActionError, Bot};

/// Book clicks per batch; Paper kicks for recipe spam past about 20 quick ones.
const MAX_PLACES: u8 = 8;

impl Bot {
    /// Lays out up to `times` crafts of `placements` from the book, returning how many it placed; 0 when no unlocked
    /// recipe makes `result` (Java id, count per craft) from them or the server placed nothing.
    pub(super) async fn java_place_from_book(&mut self, result: (i32, i32), placements: &[Placement], times: u8) -> Result<u8, ActionError> {
        let mut cells: Vec<(SlotRef, i32, u32)> = Vec::new();
        for &(from, to, n) in placements {
            let Some(item) = from.stack(&self.state).map(|s| s.network_id) else { return Ok(0) };
            match cells.iter_mut().find(|c| c.0 == to) {
                Some(cell) => cell.2 += u32::from(n),
                None => cells.push((to, item, u32::from(n))),
            }
        }
        let items: Vec<i32> = cells.iter().map(|c| c.1).collect();
        let Backend::Java(java) = &self.backend else { return Ok(0) };
        let Some(display) = java.client.book_recipe(result, &items) else { return Ok(0) };
        let places = times.min(MAX_PLACES);
        for _ in 0..places {
            self.click_pause().await?;
            let Backend::Java(java) = &mut self.backend else { return Ok(0) };
            java.client.place_recipe(display).map_err(|_| ActionError::Disconnected)?;
        }
        let want: Vec<(SlotRef, u32)> = cells.iter().map(|&(cell, _, n)| (cell, n / u32::from(times) * u32::from(places))).collect();
        let laid_out = |bot: &Bot, _: &_| want.iter().all(|&(cell, n)| cell.stack(&bot.state).is_some_and(|s| u32::from(s.count) == n)).then_some(());
        match self.wait_until(RESULT_TIMEOUT, laid_out).await {
            Ok(()) => Ok(places),
            Err(ActionError::Timeout) if want.iter().all(|(cell, _)| cell.stack(&self.state).is_none_or(|s| s.is_empty())) => Ok(0),
            Err(e) => Err(e),
        }
    }
}
