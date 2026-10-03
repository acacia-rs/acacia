//! Validates a request's ops against the tracked slots, encodes them, and predicts the result.
//!
//! `ItemStackResponse` only carries counts and stack ids, so the trackers cannot tell which item
//! lands in a slot that was empty. The plan keeps the predicted contents of every slot it touches
//! and writes them to the state once the server accepts the request; a rejected request leaves
//! the state untouched (the trackers ignore failed responses).

use acacia_client::proto::packets::ItemStackRequest as ItemStackRequestPacket;
use acacia_client::proto::types::{
    ItemStackRequestActionsItem as Action, ItemStackRequestActionsItemContent as Content, ItemStackResponsesItem,
    StackRequestSlotInfo,
};

use super::craft::Craft;
use super::request::{encode, request_packet, Texts};
use super::slot::Screen;
use super::{Op, SlotRef};
use crate::state::{GameState, ItemStack};
use crate::ActionError;

/// Stack id of a stack this request itself created or changed. Vanilla refers to such a stack by
/// the request id (servers resolve a negative id to the request's response), which is only known
/// when the request is sent.
const THIS_REQUEST: i32 = i32::MIN;

#[derive(Debug)]
pub(crate) struct Plan {
    screen: Screen,
    /// Predicted contents of every slot the ops touch.
    slots: Vec<(SlotRef, ItemStack)>,
    /// Slots an earlier action of this request changed: vanilla names their stack by the request
    /// id from then on, even once emptied (2026-10-02 capture: enchanting, recipe book).
    touched: Vec<SlotRef>,
    actions: Vec<Action>,
    texts: Texts,
    /// Results a `Create` op can put into the created-output slot.
    results: Vec<ItemStack>,
}

impl Plan {
    pub fn build(state: &GameState, screen: Screen, ops: &[Op]) -> Result<Plan, ActionError> {
        if ops.is_empty() {
            return Err(ActionError::NotPossible("no item actions".into()));
        }
        let mut plan = Plan::new(screen, Vec::with_capacity(ops.len()), Texts::default(), Vec::new());
        plan.push_all(state, ops)?;
        Ok(plan)
    }

    /// A request opened by `head`, actions without slots of their own (`BeaconPayment`), then `ops`.
    pub fn headed(state: &GameState, screen: Screen, head: Vec<Action>, ops: &[Op]) -> Result<Plan, ActionError> {
        let mut plan = Plan::new(screen, head, Texts::default(), Vec::new());
        plan.push_all(state, ops)?;
        Ok(plan)
    }

    fn new(screen: Screen, actions: Vec<Action>, texts: Texts, results: Vec<ItemStack>) -> Plan {
        Plan { screen, slots: Vec::new(), touched: Vec::new(), actions, texts, results }
    }

    /// A crafting request: the craft action and its deprecated results list, then `ops` (the
    /// `Consume`s and the move out of [`SlotRef::CREATED_OUTPUT`]).
    pub fn craft(state: &GameState, screen: Screen, craft: &Craft, ops: &[Op]) -> Result<Plan, ActionError> {
        let results: Vec<ItemStack> = craft.created().iter().map(created).collect();
        let mut plan = Plan::new(screen, craft.actions(), craft.texts.clone(), results);
        let output = plan.load(state, SlotRef::CREATED_OUTPUT)?;
        // A single result appears at once; several wait for `Create` (dragonfly `createResults`).
        if let [single] = plan.results.as_slice() {
            plan.slots[output].1 = single.clone();
        }
        plan.push_all(state, ops)?;
        Ok(plan)
    }

    pub fn request(&self, request_id: i32) -> ItemStackRequestPacket {
        let mut actions = self.actions.clone();
        for slot in actions.iter_mut().flat_map(slot_infos) {
            if slot.stack_id == THIS_REQUEST {
                slot.stack_id = request_id;
            }
        }
        request_packet(request_id, actions, self.texts.clone())
    }

    /// The predicted contents of `slot` after the request, if the plan touches it.
    pub fn predicted(&self, slot: SlotRef) -> Option<&ItemStack> {
        self.slots.iter().find(|(s, _)| *s == slot).map(|(_, stack)| stack)
    }

    /// Writes the prediction to the state, then the server's counts and stack ids on top of it.
    pub fn commit(self, state: &mut GameState, response: &ItemStackResponsesItem) {
        for (slot, mut stack) in self.slots {
            if stack.stack_network_id == Some(THIS_REQUEST) {
                stack.stack_network_id = None;
            }
            if let Some(dst) = slot.stack_mut(state) {
                *dst = stack;
            }
        }
        for container in response.containers.iter().flatten() {
            let kind = container.slot_type.container_id;
            for slot in &container.slots {
                if let Some(dst) = state.inventory.response_target(kind, slot.slot) {
                    dst.apply_response(slot);
                } else if let Some(dst) = state.containers.response_target(kind, slot.slot) {
                    dst.apply_response(slot);
                }
            }
        }
    }

    fn push_all(&mut self, state: &GameState, ops: &[Op]) -> Result<(), ActionError> {
        ops.iter().try_for_each(|&op| self.push(state, op))
    }

    fn push(&mut self, state: &GameState, op: Op) -> Result<(), ActionError> {
        match op {
            Op::Transfer { from, to, count } => {
                let (src, dst) = (self.load(state, from)?, self.load(state, to)?);
                if from == to {
                    return Err(not_possible("source and destination are the same slot"));
                }
                take_check(&self.slots[src].1, count)?;
                let dst_stack = &self.slots[dst].1;
                if !dst_stack.is_empty() && !stacks_with(dst_stack, &self.slots[src].1) {
                    return Err(not_possible("the destination holds a different item"));
                }
                self.encode(op);
                let moved = self.take(src, count);
                let dst_stack = &mut self.slots[dst].1;
                if dst_stack.is_empty() {
                    *dst_stack = moved;
                } else {
                    dst_stack.count += moved.count;
                }
            }
            Op::Swap { a, b } => {
                let (ia, ib) = (self.load(state, a)?, self.load(state, b)?);
                let (sa, sb) = (&self.slots[ia].1, &self.slots[ib].1);
                if a == b || (sa.is_empty() && sb.is_empty()) {
                    return Err(not_possible("nothing to swap"));
                }
                if !sa.is_empty() && stacks_with(sa, sb) {
                    return Err(not_possible("the stacks hold the same item; move them instead"));
                }
                self.encode(op);
                let stack = std::mem::take(&mut self.slots[ia].1);
                self.slots[ia].1 = std::mem::replace(&mut self.slots[ib].1, stack);
            }
            Op::Drop { from, count } | Op::Destroy { from, count } | Op::Consume { from, count } => {
                let src = self.load(state, from)?;
                take_check(&self.slots[src].1, count)?;
                self.encode(op);
                self.take(src, count);
            }
            Op::Create { index } => {
                let result = self.results.get(usize::from(index)).cloned().ok_or_else(|| not_possible("no such craft result"))?;
                let output = self.load(state, SlotRef::CREATED_OUTPUT)?;
                self.encode(op);
                self.slots[output].1 = result;
            }
        }
        Ok(())
    }

    fn encode(&mut self, op: Op) {
        let action = encode(op, |slot, auto| {
            if self.touched.contains(&slot) {
                return slot.slot_info_with_id(&self.screen, auto, THIS_REQUEST);
            }
            let stack = self.predicted(slot).expect("slots are loaded before encoding");
            slot.slot_info(&self.screen, auto, stack)
        });
        self.actions.push(action);
        let slots = match op {
            Op::Transfer { from, to, .. } | Op::Swap { a: from, b: to } => vec![from, to],
            Op::Drop { from, .. } | Op::Destroy { from, .. } | Op::Consume { from, .. } => vec![from],
            Op::Create { .. } => vec![SlotRef::CREATED_OUTPUT],
        };
        self.touched.extend(slots);
    }

    /// Index of `slot` in the prediction, loading it from the state on first use.
    fn load(&mut self, state: &GameState, slot: SlotRef) -> Result<usize, ActionError> {
        if let Some(i) = self.slots.iter().position(|(s, _)| *s == slot) {
            return Ok(i);
        }
        let stack = slot.stack(state).ok_or_else(|| not_possible(&format!("{slot:?} is not an open slot")))?;
        self.slots.push((slot, stack.clone()));
        Ok(self.slots.len() - 1)
    }

    /// Removes `count` items from the predicted slot and returns them as a new stack.
    fn take(&mut self, index: usize, count: u8) -> ItemStack {
        let src = &mut self.slots[index].1;
        let moved = created(&ItemStack { count: u16::from(count), ..src.clone() });
        src.count -= u16::from(count);
        if src.count == 0 {
            *src = ItemStack::default();
        }
        moved
    }
}

/// `stack` as a stack this request makes (its id is the request's until the server assigns one).
fn created(stack: &ItemStack) -> ItemStack {
    ItemStack { stack_network_id: Some(THIS_REQUEST), ..stack.clone() }
}

fn slot_infos(action: &mut Action) -> Vec<&mut StackRequestSlotInfo> {
    match &mut action.content {
        Content::Take(a) => vec![&mut a.source, &mut a.destination],
        Content::Place(a) => vec![&mut a.source, &mut a.destination],
        Content::Swap(a) => vec![&mut a.source, &mut a.destination],
        Content::Drop(a) => vec![&mut a.source],
        Content::Destroy(a) => vec![&mut a.source],
        Content::Consume(a) => vec![&mut a.source],
        _ => Vec::new(),
    }
}

/// Whether two stacks are the same item and so can merge (max stack size is not known here).
pub(crate) fn stacks_with(a: &ItemStack, b: &ItemStack) -> bool {
    a.network_id == b.network_id && a.metadata == b.metadata && a.block_runtime_id == b.block_runtime_id && a.nbt == b.nbt
}

fn take_check(stack: &ItemStack, count: u8) -> Result<(), ActionError> {
    if stack.is_empty() {
        return Err(not_possible("the source slot is empty"));
    }
    if count == 0 || u16::from(count) > stack.count {
        return Err(not_possible(&format!("cannot take {count} of {} items", stack.count)));
    }
    Ok(())
}

fn not_possible(reason: &str) -> ActionError {
    ActionError::NotPossible(reason.into())
}
