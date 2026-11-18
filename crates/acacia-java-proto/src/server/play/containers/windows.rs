//! Container bodies the root modules lack. `ContainerClose` is one body both ways; `ContainerClick` decodes
//! with its [`HashedStack`](crate::container::HashedStack)s.

use super::{read_capped_len, read_capped_string};
use crate::server::read_rest;
use crate::{Error, VarInt, Wire};

/// `ClientboundPlaceGhostRecipePacket`: the container, then a `RecipeDisplay` kept as the raw rest of the frame.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlaceGhostRecipe {
    pub container: VarInt,
    pub display: Vec<u8>,
}

impl Wire for PlaceGhostRecipe {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { container: VarInt::read(input)?, display: read_rest(input) })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.container.write(out);
        out.extend_from_slice(&self.display);
    }
}

pub const MAX_BOOK_PAGES: usize = 100;
pub const MAX_BOOK_PAGE: usize = 1024;
pub const MAX_BOOK_TITLE: usize = 32;

/// `ServerboundEditBookPacket`: the hotbar slot (an `Inventory` index), the pages as typed, and the title when
/// the player signs the book.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditBook {
    pub slot: VarInt,
    pub pages: Vec<String>,
    pub title: Option<String>,
}

impl Wire for EditBook {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let slot = VarInt::read(input)?;
        let pages = (0..read_capped_len(input, MAX_BOOK_PAGES)?).map(|_| read_capped_string(input, MAX_BOOK_PAGE)).collect::<Result<_, _>>()?;
        let title = if bool::read(input)? { Some(read_capped_string(input, MAX_BOOK_TITLE)?) } else { None };
        Ok(Self { slot, pages, title })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.slot.write(out);
        self.pages.write(out);
        self.title.is_some().write(out);
        if let Some(title) = &self.title {
            title.write(out);
        }
    }
}

// Serverbound `SetCarriedItem`: the hotbar slot, 0-8.
crate::packets::packet!(SetCarriedItem { slot: i16 });
// Serverbound: the client toggled a slot's state (a crafter's disabled slot).
crate::packets::packet!(ContainerSlotStateChanged { slot: VarInt, container: VarInt, new_state: bool });
