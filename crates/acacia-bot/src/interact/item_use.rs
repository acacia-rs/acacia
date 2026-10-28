use std::time::Instant;

use acacia_client::proto::types::Action;
use acacia_physics::{BlockPos, Vec3};

use super::confirm::{unpredict, Confirm, Expect, Verdict};
use super::geometry::{block_distance, block_min, relative, BLOCK_REACH};
use super::legacy::{self, SlotChange};
use super::wire::{self, SwingSource};
use super::{is_interactive, Face};
use crate::human::STOP_USE_ON;
use crate::reflex::Reflex;
use crate::state::ItemStack;
use crate::{ActionError, Bot};

/// Clicks of a placement the server shows no block for.
const MAX_TRIES: u32 = 3;

impl Bot {
    /// Swings the main hand (`Animate`), as a click on nothing does.
    pub fn swing(&mut self) {
        self.swing_from(None);
    }

    pub(crate) fn swing_from(&mut self, source: Option<SwingSource>) {
        self.client.send(&wire::swing(self.runtime_id(), source));
    }

    /// Uses the held item in the air (`UseItem` ClickAir): eat, drink, throw, draw a bow, raise a
    /// shield. Food finishes on its own; bows and crossbows need [`Bot::release_item`]. Geyser
    /// aims throws with the rotation of the last `PlayerAuthInput`, so physics bots should set
    /// [`crate::movement::Controls`] a tick before.
    pub fn use_item(&mut self) {
        self.client.send(&wire::click_air(self.hand()));
    }

    /// Releases the item in use (`ReleaseItem` Release): shoots a drawn bow.
    pub fn release_item(&mut self) {
        self.client.send(&wire::release(self.hand()));
    }

    /// Right-clicks the block at `pos` with the held item (`UseItem` ClickBlock): opens doors, presses
    /// buttons, uses containers. Clicks a point the eye sees on its shape, on `face` when that face shows,
    /// else on another; physics bots face it first. `NotVisible` when the block is hidden.
    pub async fn use_item_on_block(&mut self, pos: BlockPos, face: Face) -> Result<(), ActionError> {
        let distance = block_distance(self.eye_position(), pos);
        if distance > BLOCK_REACH {
            return Err(ActionError::NotPossible(format!("block {pos:?} is {distance:.2} blocks away")));
        }
        let (face, aim) = self.visible_aim(pos, Some(face))?;
        self.look_at(aim).await?;
        self.click_block(pos, face, relative(pos, aim), SwingSource::Interact)
    }

    /// Clicks the door, gate or trapdoor at `pos` and waits for the server to show it `open` (or closed). One
    /// click only: a second would toggle it back were the first just late.
    pub(crate) async fn door_confirmed(&mut self, pos: BlockPos, open: bool) -> Result<(), ActionError> {
        let distance = block_distance(self.eye_position(), pos);
        if distance > BLOCK_REACH {
            return Err(ActionError::NotPossible(format!("door {pos:?} is {distance:.2} blocks away")));
        }
        let (face, aim) = self.visible_aim(pos, None)?;
        self.look_at(aim).await?;
        self.admit_action().await?;
        let mut c = self.confirmation(pos, Expect::Prop("open_bit", if open { "1" } else { "0" }), 1);
        let clicked = self.click_block(pos, face, relative(pos, aim), SwingSource::Interact);
        let verdict = match clicked {
            Ok(()) => self.await_confirm(pos, &mut c, Instant::now()).await,
            Err(e) => Err(e),
        };
        self.governor.settled();
        match verdict? {
            Verdict::Done => Ok(()),
            _ => Err(ActionError::Rejected(format!("the door at {pos:?} did not move"))),
        }
    }

    /// Right-clicks the block at `pos` (on `face` when it shows) and waits for the server to show the cell
    /// changed: a use toggle (`open_bit`, a repeater's delay) or a held tool's work (tilled dirt). One click
    /// only, as [`Self::door_confirmed`].
    pub(crate) async fn use_confirmed(&mut self, pos: BlockPos, face: Face) -> Result<(), ActionError> {
        let distance = block_distance(self.eye_position(), pos);
        if distance > BLOCK_REACH {
            return Err(ActionError::NotPossible(format!("block {pos:?} is {distance:.2} blocks away")));
        }
        let before = self.block_at(pos).map(|(id, _)| id).ok_or_else(|| ActionError::NotPossible(format!("{pos:?} is not loaded")))?;
        let (face, aim) = self.visible_aim(pos, Some(face))?;
        self.look_at(aim).await?;
        self.admit_action().await?;
        let mut c = self.confirmation(pos, Expect::Replaced(before), 1);
        let verdict = match self.click_block(pos, face, relative(pos, aim), SwingSource::Interact) {
            Ok(()) => self.await_confirm(pos, &mut c, Instant::now()).await,
            Err(e) => Err(e),
        };
        self.governor.settled();
        match verdict? {
            Verdict::Done => Ok(()),
            _ => Err(ActionError::Rejected(format!("using {pos:?} changed nothing"))),
        }
    }

    /// Places the held block or item (sign, bed, boat...) against `face` of the block at `against`.
    /// Waits until the server shows a block in the target cell, clicking again (up to 3 tries) while it
    /// stays empty past the governor's timeout; `Rejected` after that. Items that make entities, like
    /// boats, are not waited for.
    pub async fn place_block(&mut self, against: BlockPos, face: Face) -> Result<(), ActionError> {
        self.place_block_at(against, face, face.click_offset()).await
    }

    /// [`Bot::place_block`] clicking at `click` (relative to `against`'s minimum corner), which picks a slab's
    /// half or a stair's upside-down bit. A half slab in the target cell may be clicked into to make it double.
    pub async fn place_block_at(&mut self, against: BlockPos, face: Face, click: Vec3) -> Result<(), ActionError> {
        let (target, half_slab) = self.aim_placement(against, face, click).await?;
        let makes_entity = self.held_name().is_some_and(|n| n.ends_with("boat") || n.ends_with("minecart"));
        if makes_entity || self.block_at(target).is_none() {
            return self.click_block(against, face, click, SwingSource::Build);
        }
        self.admit_action().await?;
        let result = self.place_confirmed(against, face, click, half_slab).await;
        self.governor.settled();
        result.map_err(|e| match e {
            ActionError::Rejected(why) => ActionError::Rejected(format!("placing at {target:?}: {why}")),
            e => e,
        })
    }

    /// Checks a placement against `face` of `against` and turns to the click point; returns the target cell and
    /// the half slab already in it, if any.
    pub(crate) async fn aim_placement(&mut self, against: BlockPos, face: Face, click: Vec3) -> Result<(BlockPos, Option<u32>), ActionError> {
        if self.state.inventory.held().is_empty() {
            return Err(ActionError::NotPossible("nothing held to place".into()));
        }
        let target = face.adjacent(against);
        let mut half_slab = None;
        if let Some((id, s)) = self.block_at(target)
            && !s.is_air()
            && !s.is_liquid()
        {
            if !s.name.ends_with("_slab") && !adds_to(s.name, self.held_name()) {
                return Err(ActionError::NotPossible(format!("{target:?} is occupied by {}", s.name)));
            }
            half_slab = Some(id);
        }
        if self.block_at(against).is_some_and(|(_, s)| s.is_air()) {
            return Err(ActionError::NotPossible(format!("nothing to place against at {against:?}")));
        }
        let sneaking = self.movement.as_ref().is_some_and(|m| m.controls.sneak);
        if !sneaking && self.block_at(against).is_some_and(|(_, s)| is_interactive(s)) {
            return Err(ActionError::NotPossible(format!("clicking {against:?} would use it, not place against it")));
        }
        let distance = block_distance(self.eye_position(), against);
        if distance > BLOCK_REACH {
            return Err(ActionError::NotPossible(format!("block {against:?} is {distance:.2} blocks away")));
        }
        let m = block_min(against);
        let point = [m[0] + click[0], m[1] + click[1], m[2] + click[2]];
        let eye = self.eye_position();
        let into = half_slab.map(|_| target);
        if self.sight().is_some_and(|s| !s.sees_face_into(eye, against, face, point, into)) {
            return Err(ActionError::NotVisible(format!("{face:?} of {against:?} is hidden from the eye")));
        }
        let t = Instant::now();
        self.look_at(point).await?;
        self.act_timing.add(crate::act_timing::Phase::Aim, t.elapsed());
        Ok((target, half_slab))
    }

    /// Clicks, then waits for the server's echo; clicks again while the cell stays empty. A refused click's
    /// predicted stack change is undone (see [`unpredict`]).
    async fn place_confirmed(&mut self, against: BlockPos, face: Face, click: Vec3, half_slab: Option<u32>) -> Result<(), ActionError> {
        let target = face.adjacent(against);
        let expect = half_slab.map_or(Expect::Solid, Expect::Replaced);
        let mut confirm: Option<Confirm> = None;
        loop {
            let (slot, before) = (self.state.inventory.selected_hotbar_slot, self.state.inventory.held().clone());
            if before.is_empty() {
                return Err(ActionError::NotPossible("nothing held to place".into()));
            }
            let c = match &mut confirm {
                Some(c) => {
                    c.retried(self.governor.now(), self.governor.timeout_ticks());
                    self.governor.sent(true);
                    c
                }
                None if half_slab.is_some() => confirm.insert(self.confirmation(target, expect, MAX_TRIES)),
                None => confirm.insert(self.confirmation(target, expect, MAX_TRIES).predicted()),
            };
            self.click_block(against, face, click, SwingSource::Build)?;
            let predict = before.block_runtime_id != 0 && half_slab.is_none();
            let replaced = predict.then(|| self.predict_block(target, before.block_runtime_id)).flatten();
            let predicted = self.state.inventory.held().clone();
            let verdict = self.await_confirm(target, c, Instant::now()).await?;
            if verdict != Verdict::Done {
                if let Some(old) = replaced.filter(|_| c.unechoed()) {
                    self.unpredict_block(target, old);
                }
                let main = &mut self.state.inventory.main[usize::from(slot)];
                if let Some(back) = unpredict(main, &predicted, &before) {
                    tracing::debug!(?target, "placement refused; undoing the predicted stack change");
                    *main = back;
                }
            }
            match verdict {
                Verdict::Done => return Ok(()),
                Verdict::Retry => {}
                Verdict::Failed(why) => return Err(ActionError::Rejected(why)),
                Verdict::Wait => unreachable!("await_confirm decides"),
            }
        }
    }

    pub(super) fn held_name(&self) -> Option<&str> {
        self.state.items.name(self.state.inventory.held().network_id)
    }

    /// Vanilla's block click: StartItemUseOn, the swing, the ClickBlock (placing: with the held
    /// stack's predicted change, then the new stack in MobEquipment), StopItemUseOn a moment later.
    pub(super) fn click_block(&mut self, pos: BlockPos, face: Face, click: Vec3, source: SwingSource) -> Result<(), ActionError> {
        let block = self.block_wire_id(pos)?;
        tracing::debug!(?pos, ?face, block, "click block");
        let held = self.state.inventory.held().clone();
        let is_sign = self.held_name().is_some_and(|n| n.ends_with("sign"));
        let result = use_on_result(&held, is_sign, pos, face);
        if self.reflexes.hovered.take().is_some() {
            self.client.send(&wire::mouse_over_nothing());
        }
        self.client.send(&wire::item_use_on(self.runtime_id(), Action::StartItemUseOn, pos, result, face as i32));
        self.swing_from(Some(source));
        let packet = wire::click_block(self.hand(), pos, face, click, block);
        if source != SwingSource::Build {
            self.client.send(&packet);
        } else {
            let slot = self.state.inventory.selected_hotbar_slot;
            let left = one_used(&held);
            // Capture: sign 2 -> 1 had a legacy id, bed and boat 1 -> 0 had none.
            let legacy_id = (!left.is_empty()).then(|| self.reflexes.next_legacy_id());
            self.client.send(&legacy::with_slot_change(packet, legacy_id, &SlotChange { slot, old: &held, new: &left }));
            self.state.inventory.main[usize::from(slot)] = left;
            if is_sign {
                self.client.send(&wire::click_air(self.hand()));
            }
            self.equip_now();
        }
        self.later(STOP_USE_ON, Reflex::StopUseOn(result));
        Ok(())
    }
}

/// Vanilla's `result_position`: the cell a placement would fill when the hand is empty or holds a
/// block or sign, else the clicked block (beds, boats, tools, other items).
pub(crate) fn use_on_result(held: &ItemStack, is_sign: bool, pos: BlockPos, face: Face) -> BlockPos {
    if held.is_empty() || held.block_runtime_id != 0 || is_sign {
        face.adjacent(pos)
    } else {
        pos
    }
}

/// Placing `held` into a cell holding `cell` adds one to it (candles, sea pickles, turtle eggs).
pub(crate) fn adds_to(cell: &str, held: Option<&str>) -> bool {
    held == Some(cell) && (cell.ends_with("candle") || matches!(cell, "minecraft:sea_pickle" | "minecraft:turtle_egg"))
}

/// The stack left after placing one.
pub(crate) fn one_used(held: &ItemStack) -> ItemStack {
    match held.count {
        0 | 1 => ItemStack::default(),
        n => ItemStack { count: n - 1, ..held.clone() },
    }
}
