//! Getting into a boat at the launch bank (an empty one in reach, else one placed from the hotbar) and, at the far
//! bank, breaking it and picking the item up.

use std::time::Duration;

use acacia_client::proto::types::Vec3f;
use acacia_physics::BlockPos;

use super::grid::WaterGrid;
use crate::human::{BOAT_PUNCH, HOTBAR_SWITCH};
use crate::interact::Face;
use crate::pathfind::Terrain;
use crate::pathfind::{Goal, GotoOpts};
use crate::state::{ITEM_KIND, Inventory};
use crate::{ActionError, Bot};

const BOAT_KINDS: &[&str] = &["minecraft:boat", "minecraft:chest_boat"];
/// A found boat further than this from the eye is not reached for.
const FIND_REACH: f32 = 3.0;
/// Water cells this far out (blocks, horizontal) are tried for a placement.
const PLACE_REACH: i32 = 3;
/// Centre offset (blocks, x or z) at which a boat's box (1.4 wide) clears the bot's (0.6), with a margin.
const PLAYER_CLEAR: f32 = 1.1;
/// How long the server has to show a placed boat, or a broken one's drop.
const SHOW_TIMEOUT: Duration = Duration::from_secs(2);
/// Hits before a boat that will not break is left.
const MAX_HITS: u32 = 12;

impl Bot {
    /// Boards a boat for a leg towards `toward` and returns its runtime id: the nearest empty boat within reach,
    /// else one placed from the hotbar on the water beside the bot.
    pub(super) async fn board_boat(&mut self, toward: BlockPos) -> Result<u64, ActionError> {
        if let Some(id) = self.boat_in_reach() {
            match self.mount(id).await {
                Ok(_) => return Ok(id),
                Err(e) => tracing::debug!(error = %e, "found boat refused; placing one"),
            }
        }
        let Some(slot) = self.hotbar_boat() else {
            let seen: Vec<_> = self.state.entities.iter().map(|e| (e.kind.clone(), e.feet())).collect();
            tracing::debug!(?seen, eye = ?self.eye_position(), "no boat to board");
            return Err(ActionError::NotPossible("no boat in reach or in the hotbar".into()));
        };
        if usize::from(self.state.inventory.selected_hotbar_slot) != slot {
            self.select_hotbar(slot as u8)?;
            let delay = self.human.between(HOTBAR_SWITCH);
            self.pause(delay).await?;
        }
        let water = self.launch_water(toward).ok_or_else(|| ActionError::NotPossible("no open water beside the bot to launch a boat on".into()))?;
        let before: Vec<u64> = self.boats().map(|(id, _)| id).collect();
        self.place_block(water, Face::Up).await?;
        let near = Vec3f { x: water[0] as f32 + 0.5, y: water[1] as f32 + 1.0, z: water[2] as f32 + 0.5 };
        let placed = move |bot: &Bot, _: &_| bot.boats().find(|(id, e)| !before.contains(id) && e.distance_sq(&near) < 9.0).map(|(id, _)| id);
        let id = match self.wait_until(SHOW_TIMEOUT, placed).await {
            Err(ActionError::Timeout) => {
                let seen: Vec<_> = self.boats().map(|(id, e)| (id, e.feet())).collect();
                tracing::debug!(?water, ?seen, held = ?self.state.inventory.held(), "no placed boat");
                return Err(ActionError::Rejected(format!("no boat appeared on the water at {water:?}")));
            }
            other => other?,
        };
        self.mount(id).await?;
        Ok(id)
    }

    /// After getting off: hits the boat until it breaks, then picks up what it dropped.
    pub(super) async fn collect_boat(&mut self, id: u64) -> Result<(), ActionError> {
        let before: Vec<u64> = self.drops().collect();
        for _ in 0..MAX_HITS {
            if self.state.entities.get(id).is_none() {
                break;
            }
            self.attack(id).await?;
            let gap = self.human.between(BOAT_PUNCH);
            self.pause(gap).await?;
        }
        if self.state.entities.get(id).is_some() {
            return Err(ActionError::Rejected(format!("the boat did not break after {MAX_HITS} hits")));
        }
        let dropped = move |bot: &Bot, _: &_| bot.drops().find(|d| !before.contains(d));
        let drop = match self.wait_until(SHOW_TIMEOUT, dropped).await {
            Err(ActionError::Timeout) => return Ok(()),
            other => other?,
        };
        if let Some(e) = self.state.entities.get(drop) {
            let f = e.feet();
            self.goto_with(Goal::Near([f.x.floor() as i32, f.y.floor() as i32, f.z.floor() as i32], 1.0), GotoOpts::default()).await?;
            let picked = move |bot: &Bot, _: &_| bot.state.entities.get(drop).is_none().then_some(());
            let _ = self.wait_until(SHOW_TIMEOUT, picked).await;
        }
        Ok(())
    }

    fn boats(&self) -> impl Iterator<Item = (u64, &crate::state::Entity)> {
        self.state.entities.iter().filter(|e| BOAT_KINDS.contains(&e.kind.as_str())).map(|e| (e.runtime_id, e))
    }

    fn drops(&self) -> impl Iterator<Item = u64> + '_ {
        let boat_item = |name: &str| name.ends_with("boat");
        self.state.entities.iter().filter(move |e| {
            e.kind == ITEM_KIND && e.item.as_ref().and_then(|s| self.state.items.name(s.network_id)).is_some_and(boat_item)
        }).map(|e| e.runtime_id)
    }

    fn boat_in_reach(&self) -> Option<u64> {
        let eye = self.eye_position();
        let eye = Vec3f { x: eye[0], y: eye[1], z: eye[2] };
        let riders: Vec<i64> = self.state.riding.vehicle.iter().map(|v| v.unique_id).collect();
        self.boats().filter(|(_, e)| !riders.contains(&e.unique_id) && e.distance_sq(&eye) <= FIND_REACH * FIND_REACH).min_by(|a, b| a.1.distance_sq(&eye).total_cmp(&b.1.distance_sq(&eye))).map(|(id, _)| id)
    }

    fn hotbar_boat(&self) -> Option<usize> {
        let hotbar = self.state.inventory.main.iter().take(usize::from(Inventory::HOTBAR_SLOTS));
        let name = |s: &crate::state::ItemStack| self.state.items.name(s.network_id).unwrap_or_default().to_owned();
        let boats: Vec<(usize, String)> = hotbar.enumerate().filter(|(_, s)| !s.is_empty()).map(|(i, s)| (i, name(s))).filter(|(_, n)| n.ends_with("boat")).collect();
        // A plain boat before a chest boat: the chest boat's cargo is not handled.
        boats.iter().find(|(_, n)| !n.ends_with("chest_boat")).or(boats.first()).map(|(i, _)| *i)
    }

    /// The still-water cell a block below the feet, with room for a boat that clears the bot, nearest the bot
    /// (ties: nearest `toward`). BDS refused every placement 3.5 blocks out and took one at 2.6 (2026-10-28).
    fn launch_water(&self, toward: BlockPos) -> Option<BlockPos> {
        let feet = self.movement.as_ref()?.position()?;
        let world = self.world.as_ref()?;
        let (view, registry) = (world.view()?, world.registry()?);
        let terrain = Terrain::new(view, registry.as_ref());
        let y = feet[1].floor() as i32 - 1;
        let grid = WaterGrid::new(&terrain, y);
        let (fx, fz) = (feet[0].floor() as i32, feet[2].floor() as i32);
        let mut best: Option<((f32, f32), BlockPos)> = None;
        for x in fx - PLACE_REACH..=fx + PLACE_REACH {
            for z in fz - PLACE_REACH..=fz + PLACE_REACH {
                let (cx, cz) = (x as f32 + 0.5, z as f32 + 0.5);
                // BDS refuses a boat whose box overlaps an entity, the placing player included.
                let clears_bot = (cx - feet[0]).abs() >= PLAYER_CLEAR || (cz - feet[2]).abs() >= PLAYER_CLEAR;
                if !clears_bot || !grid.clear(cx, cz, 0.0) {
                    continue;
                }
                let key = ((cx - feet[0]).hypot(cz - feet[2]), ((toward[0] - x) as f32).hypot((toward[2] - z) as f32));
                if best.is_none_or(|(b, _)| key < b) {
                    best = Some((key, [x, y, z]));
                }
            }
        }
        best.map(|(_, p)| p)
    }
}
