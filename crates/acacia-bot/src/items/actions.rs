use std::time::Duration;

use acacia_client::proto::packets::{ContainerClose, Interact, InteractActionId};
use acacia_client::proto::types::{ItemStackRequestActionsItem as Action, ItemStackResponsesItem, WindowID, WindowType};
use acacia_world::BlockAccess;

use super::craft::Craft;
use super::plan::Plan;
use super::quick::quick_move_ops;
use super::request::{check_status, response_for};
use super::slot::{open_container, BlockKind, Screen};
use super::{Op, SlotRef};
use crate::state::GameState;
use crate::{ActionError, Bot};

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(3);
/// How long to wait for the server to confirm the own inventory opened. BDS may not answer, so
/// running out of time here is not an error.
const OPEN_TIMEOUT: Duration = Duration::from_secs(1);

/// Item transfers through server-authoritative `ItemStackRequest`s.
///
/// Every method waits for the server's `ItemStackResponse` (3 s) and then updates the tracked
/// slots, including items landing in empty slots; a rejection returns [`ActionError::Rejected`]
/// and leaves the tracked state as it was. If no container is open, each call opens the player's
/// own inventory screen around the request and closes it afterwards: Geyser drops item requests
/// while no screen is open.
impl Bot {
    /// Moves `count` items from one slot onto an empty slot or a stack of the same item.
    pub async fn move_item(&mut self, from: SlotRef, to: SlotRef, count: u8) -> Result<(), ActionError> {
        self.item_stack_request(&[Op::Transfer { from, to, count }]).await
    }

    /// Exchanges the contents of two slots (which must not hold the same item).
    pub async fn swap_items(&mut self, a: SlotRef, b: SlotRef) -> Result<(), ActionError> {
        self.item_stack_request(&[Op::Swap { a, b }]).await
    }

    /// Throws `count` items from a slot into the world.
    pub async fn drop_item(&mut self, from: SlotRef, count: u8) -> Result<(), ActionError> {
        self.item_stack_request(&[Op::Drop { from, count }]).await
    }

    /// Deletes `count` items from a slot (creative mode only).
    pub async fn destroy_item(&mut self, from: SlotRef, count: u8) -> Result<(), ActionError> {
        self.item_stack_request(&[Op::Destroy { from, count }]).await
    }

    /// Picks the whole stack up into the cursor: a left click, which is how server GUI menus
    /// (chest-based shops through Geyser) see a button press. The server usually answers by
    /// resetting the menu and cursor with `InventorySlot`/`InventoryContent`.
    pub async fn pick_up(&mut self, from: SlotRef) -> Result<(), ActionError> {
        let count = from.stack(&self.state).map_or(0, |s| u8::try_from(s.count).unwrap_or(u8::MAX));
        self.item_stack_request(&[Op::Transfer { from, to: SlotRef::Cursor, count }]).await
    }

    /// Shift-click: moves the whole stack to the other side (container ↔ player inventory, or
    /// hotbar ↔ inventory). Returns the slots it went to.
    pub async fn quick_move(&mut self, from: SlotRef) -> Result<Vec<SlotRef>, ActionError> {
        let ops = quick_move_ops(&self.state, from)?;
        self.item_stack_request(&ops).await?;
        Ok(ops.iter().filter_map(|op| if let Op::Transfer { to, .. } = op { Some(*to) } else { None }).collect())
    }

    /// Moves a stack of the open container into the player's inventory.
    pub async fn move_to_inventory(&mut self, container_slot: u8) -> Result<Vec<SlotRef>, ActionError> {
        self.quick_move(SlotRef::Container(container_slot)).await
    }

    /// Moves the whole stack in main inventory slot `inventory_slot` (0-35) into `container_slot`.
    pub async fn put_in_container(&mut self, inventory_slot: u8, container_slot: u8) -> Result<(), ActionError> {
        let from = SlotRef::Main(inventory_slot);
        let count = from.stack(&self.state).map_or(0, |s| u8::try_from(s.count).unwrap_or(u8::MAX));
        self.move_item(from, SlotRef::Container(container_slot), count).await
    }

    /// Moves every stack of the open container into the player's inventory, stopping when it is
    /// full. Returns how many stacks moved.
    pub async fn take_all(&mut self) -> Result<usize, ActionError> {
        let Some(open) = open_container(&self.state) else {
            return Err(ActionError::NotPossible("no container is open".into()));
        };
        let slots: Vec<u8> = open.iter().filter_map(|(i, _)| u8::try_from(i).ok()).collect();
        let mut moved = 0;
        for slot in slots {
            match self.move_to_inventory(slot).await {
                Ok(_) => moved += 1,
                Err(ActionError::NotPossible(_)) if moved > 0 => break,
                Err(e) => return Err(e),
            }
        }
        Ok(moved)
    }

    /// Sends `ops` as one `ItemStackRequest` and applies the result once the server accepts it.
    pub async fn item_stack_request(&mut self, ops: &[Op]) -> Result<(), ActionError> {
        self.run_plan(|state, screen| Plan::build(state, screen, ops)).await
    }

    /// Sends a crafting request: `craft`'s actions, then `ops`.
    pub(crate) async fn craft_request(&mut self, craft: &Craft, ops: &[Op]) -> Result<(), ActionError> {
        self.run_plan(|state, screen| Plan::craft(state, screen, craft, ops)).await
    }

    /// Sends `head` (see [`Plan::headed`]) followed by `ops`.
    pub(crate) async fn headed_request(&mut self, head: Vec<Action>, ops: &[Op]) -> Result<(), ActionError> {
        self.run_plan(|state, screen| Plan::headed(state, screen, head.clone(), ops)).await
    }

    async fn run_plan(&mut self, build: impl Fn(&GameState, Screen) -> Result<Plan, ActionError>) -> Result<(), ActionError> {
        build(&self.state, self.screen())?;
        let opened = self.open_own_inventory_screen().await?;
        let result = self.send_plan(&build).await;
        if opened {
            self.close_own_inventory_screen();
        }
        result
    }

    async fn send_plan(&mut self, build: impl Fn(&GameState, Screen) -> Result<Plan, ActionError>) -> Result<(), ActionError> {
        // Rebuilt after the screen opened: the server may have resent slots meanwhile.
        let plan = build(&self.state, self.screen())?;
        let id = self.send_request(&plan);
        let response = self.wait_until(RESPONSE_TIMEOUT, |_, packet| response_for(packet, id)).await?;
        self.finish_request(plan, &response)
    }

    /// Builds `ops` for the current screen and sends them without waiting; the caller matches the
    /// response with `response_for` and hands it to [`Bot::finish_request`].
    pub(crate) fn send_ops(&mut self, ops: &[Op]) -> Result<(Plan, i32), ActionError> {
        let plan = Plan::build(&self.state, self.screen(), ops)?;
        let id = self.send_request(&plan);
        Ok((plan, id))
    }

    /// Applies an accepted request's prediction and the server's slot updates.
    pub(crate) fn finish_request(&mut self, plan: Plan, response: &ItemStackResponsesItem) -> Result<(), ActionError> {
        tracing::debug!(?response, "item stack response");
        check_status(response)?;
        plan.commit(&mut self.state, response);
        Ok(())
    }

    fn send_request(&mut self, plan: &Plan) -> i32 {
        let id = self.request_ids.next();
        let request = plan.request(id);
        tracing::debug!(?request, "item stack request");
        self.client.send(&request);
        id
    }

    fn screen(&self) -> Screen {
        Screen::of(&self.state, self.container_block())
    }

    fn container_block(&self) -> BlockKind {
        let (Some(world), Some(pos)) = (self.world.as_ref(), open_container(&self.state).and_then(|c| c.position.as_ref())) else {
            return BlockKind::Other;
        };
        let (Some(view), Some(registry)) = (world.view(), world.registry()) else { return BlockKind::Other };
        registry.get(view.block(pos.x, pos.y, pos.z)).map_or(BlockKind::Other, |b| BlockKind::from_name(b.name))
    }

    /// Opens the own inventory screen unless some container is open; true if it did.
    pub(crate) async fn open_own_inventory_screen(&mut self) -> Result<bool, ActionError> {
        if self.state.containers.open.is_some() {
            return Ok(false);
        }
        self.send_open_inventory();
        match self.wait_until(OPEN_TIMEOUT, |bot, _| bot.state.containers.open.is_some().then_some(())).await {
            Ok(()) | Err(ActionError::Timeout) => Ok(true),
            Err(e) => Err(e),
        }
    }

    pub(crate) fn send_open_inventory(&mut self) {
        self.client.send(&Interact {
            action_id: InteractActionId::OpenInventory,
            target_entity_id: self.state.player.runtime_entity_id,
            has_position: false,
            position: None,
        });
    }

    pub(crate) fn close_own_inventory_screen(&mut self) {
        // Vanilla closes the window id the server opened the screen as, with window type None.
        let window_id = self.state.containers.open.as_ref().map_or(WindowID::Inventory, |c| WindowID::from_raw(i64::from(c.window_id)));
        self.client.send(&ContainerClose { window_id, window_type: WindowType::None, server: false });
        // Cleared now rather than on the server's echo, so the next request reopens the screen.
        self.state.containers.open = None;
    }
}
