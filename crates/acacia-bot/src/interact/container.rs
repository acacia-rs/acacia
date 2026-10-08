use std::time::Duration;

use acacia_client::proto::packets::{ContainerClose, Interact, InteractActionId};
use acacia_client::proto::types::{WindowID, WindowType};
use acacia_client::proto::{Packet, RawPacket};
use acacia_physics::BlockPos;

use super::geometry::facing_face;
use crate::state::{Container, Inventory};
use crate::{ActionError, Bot};

const CONTAINER_TIMEOUT: Duration = Duration::from_secs(3);
/// How long to wait for a container's contents after it opens; empty GUIs may never send any.
const CONTENTS_TIMEOUT: Duration = Duration::from_secs(1);

impl Bot {
    /// Opens the container block at `pos` (chest, barrel, furnace, shop GUI block...) by clicking
    /// the face towards the eye, and waits for the server's `ContainerOpen`. An open window
    /// (including the own inventory) is closed first.
    pub async fn open_container_at(&mut self, pos: BlockPos) -> Result<Container, ActionError> {
        if self.state.containers.open.is_some() {
            self.close_container().await?;
        }
        let face = facing_face(self.eye_position(), pos);
        self.use_item_on_block(pos, face).await?;
        let opened = self.wait_until(CONTAINER_TIMEOUT, |bot, _| bot.open_container().cloned()).await?;
        // Contents follow ContainerOpen in a separate InventoryContent; slot actions need them.
        let filled = |bot: &Bot, _: &RawPacket| bot.open_container().filter(|c| !c.slots.is_empty()).cloned();
        match self.wait_until(CONTENTS_TIMEOUT, filled).await {
            Ok(container) => Ok(container),
            Err(ActionError::Timeout) => Ok(self.open_container().cloned().unwrap_or(opened)),
            Err(e) => Err(e),
        }
    }

    /// Closes the open container (`ContainerClose`, not server-initiated) and waits for the
    /// server's confirming `ContainerClose`.
    pub async fn close_container(&mut self) -> Result<(), ActionError> {
        let Some(open) = self.state.containers.open.as_ref() else {
            return Err(ActionError::NotPossible("no container is open".into()));
        };
        let packet = close_packet(open);
        self.client.send(&packet);
        self.wait_until(CONTAINER_TIMEOUT, |_, p| (p.id == ContainerClose::ID).then_some(())).await
    }

    /// Opens the player's own inventory screen (`Interact` OpenInventory). The server answers with
    /// a `ContainerOpen` for window 0, which shows up in `state().containers`.
    pub fn open_inventory(&mut self) {
        self.client.send(&Interact {
            action_id: InteractActionId::OpenInventory,
            target_entity_id: self.runtime_id(),
            has_position: false,
            position: None,
        });
    }

    /// The open container, unless it is the player's own inventory.
    pub fn open_container(&self) -> Option<&Container> {
        self.state.containers.open.as_ref().filter(|c| !Inventory::is_player_window(c.window_id))
    }
}

/// The client's close: vanilla sends no window type, and names no window for the trade screen
/// (the server still echoes the real one).
fn close_packet(open: &Container) -> ContainerClose {
    let window_id = if open.window_type == WindowType::Trading { WindowID::None } else { WindowID::from_raw(i64::from(open.window_id)) };
    ContainerClose { window_id, window_type: WindowType::None, server: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_names_the_window_but_not_its_type_and_not_the_trade_screen() {
        let open = |window_type| Container { window_id: 6, window_type, position: None, entity: None, slots: Vec::new() };
        let beacon = close_packet(&open(WindowType::Beacon));
        assert_eq!((beacon.window_id.to_raw(), beacon.window_type, beacon.server), (6, WindowType::None, false));
        assert_eq!(close_packet(&open(WindowType::Trading)).window_id, WindowID::None);
    }
}
