//! The player's figure in the older inventory layout (the Bedrock look's), drawn as Java's
//! `InventoryScreen` asks for it: a picture over the panel's box, facing the mouse.

use acacia_ui::java::container::entity::{Figure, follows_mouse};
use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::{Quad, inventory};

use super::java_gui::PictureRequest;

/// `extractEntityInInventoryFollowsMouse(x + 26, y + 8, x + 75, y + 78, 30, 0.0625, mouse)`.
pub fn inventory_figure(gui: [f32; 2], mouse: [f32; 2], scale: f32) -> Option<(PictureRequest, Quad)> {
    let [x, y] = inventory::origin(inventory::Layout::Player, gui).map(|v| v as i32);
    let player = Figure { entity: "minecraft:player".into(), bb_height: 1.8, scale: Some(1.0), fall_flying: false };
    let mut calls = Gui::new(gui[0] as i32, gui[1] as i32);
    follows_mouse(&mut calls, [x + 26, y + 8, x + 75, y + 78], 30, 0.0625, mouse, &player);
    let GuiDraw::Picture(picture) = calls.draws.pop()? else { return None };
    let window = [x + 26, y + 8, x + 75, y + 78].map(|v| v as f32 * scale);
    Some((PictureRequest { picture, window, gui_scale: scale }, Quad::picture(window)))
}
