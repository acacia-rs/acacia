//! Workstation actions besides clicks, queued like them for the next [`Client::step`].

use acacia_java_proto::VarInt;
use acacia_java_proto::ids::play::serverbound as to;
use acacia_java_proto::packets::{ContainerButtonClick, RenameItem, SelectTrade, SetBeacon};

use crate::frame::Error;
use crate::join::Client;

impl Client {
    /// The anvil's name field (`ServerboundRenameItemPacket`); the server recomputes the result.
    pub fn rename_item(&mut self, name: &str) -> Result<(), Error> {
        self.queue(to::RENAME_ITEM, &RenameItem { name: name.to_owned() })
    }

    /// A button of the open menu: an enchanting option, a stonecutter or loom entry, a lectern page.
    pub fn menu_button(&mut self, button: i32) -> Result<(), Error> {
        let (container, _) = self.inventory.menu_ids();
        self.queue(to::CONTAINER_BUTTON_CLICK, &ContainerButtonClick { container: VarInt(container), button: VarInt(button) })
    }

    /// The stonecutter button that turns `input` into `result`: its index among the recipes taking `input`.
    pub fn stonecutter_button(&self, input: i32, result: i32) -> Option<i32> {
        let mut choices = self.stonecutter.iter().filter(|r| r.input.accepts(input));
        choices.position(|r| r.result.is_some_and(|(item, _)| item == result)).and_then(|i| i32::try_from(i).ok())
    }

    /// Picks a villager's offer: the server moves its cost from the inventory into the trade slots.
    pub fn select_trade(&mut self, offer: i32) -> Result<(), Error> {
        self.queue(to::SELECT_TRADE, &SelectTrade { offer: VarInt(offer) })
    }

    /// Confirms the open beacon's powers (`mob_effect` ids); the server takes the payment.
    pub fn set_beacon(&mut self, primary: i32, secondary: Option<i32>) -> Result<(), Error> {
        self.queue(to::SET_BEACON, &SetBeacon { primary: Some(VarInt(primary)), secondary: secondary.map(VarInt) })
    }
}
