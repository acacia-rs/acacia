//! What the item frames on walls hold: read from their block entities on the bot's thread, drawn
//! on the window's. A framed map's picture is asked of the server once.

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use acacia_bot::Bot;
use acacia_bot::proto::nbt::{Nbt, Value};
use acacia_bot::proto::packets::MapInfoRequest;
use acacia_bot::state::MapImage;
use acacia_render::entity::{EntityInstance, Skin};
use acacia_render::item::{ItemKey, framed, map};
use glam::{DVec3, IVec3};

use crate::smooth::Smoother;

struct Framed {
    block: [i32; 3],
    hung: framed::Hung,
    /// `ItemRotation`, degrees.
    turn: f32,
    key: ItemKey,
    enchanted: bool,
    /// A filled map's picture, once the server has sent it.
    map: Option<Arc<MapImage>>,
}

/// Pictures kept as skins: a wall of maps and some more.
const PICTURES_KEPT: usize = 64;

thread_local! {
    /// The bot thread's: the maps already asked for.
    static ASKED: RefCell<HashSet<i64>> = RefCell::new(HashSet::new());
    /// The window thread's: framed pictures and their skins, the newest first.
    static PICTURES: RefCell<Vec<(Arc<MapImage>, Arc<Skin>)>> = const { RefCell::new(Vec::new()) };
}

/// The frames last read, handed from the bot's thread to the window's.
static SHOWN: Mutex<Vec<Framed>> = Mutex::new(Vec::new());

/// Reads the wall frames holding something for [`instances`].
pub fn publish(bot: &Bot) {
    let frames = bot.state().block_entities.iter().filter_map(|(block, nbt)| framed_item(bot, *block, nbt)).collect();
    *SHOWN.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = frames;
}

fn framed_item(bot: &Bot, block: [i32; 3], nbt: &Nbt) -> Option<Framed> {
    let item = nbt.value.get("Item")?;
    let Value::String(name) = item.get("Name")? else { return None };
    let view = bot.world()?.view()?;
    let state = view.world().registry().get(acacia_world::BlockAccess::block(view, block[0], block[1], block[2]))?;
    // As `acacia_render::blocks::model` turns the frame's block model.
    let (yaw, tilt) = match state.property("facing_direction")? {
        "0" => (0.0, -90.0),
        "1" => (0.0, 90.0),
        "2" => (180.0, 0.0),
        "4" => (90.0, 0.0),
        "5" => (270.0, 0.0),
        _ => (0.0, 0.0),
    };
    let aux = match item.get("Damage") {
        Some(Value::Short(damage)) => *damage as u32,
        _ => 0,
    };
    let turn = match nbt.value.get("ItemRotation") {
        Some(Value::Float(degrees)) => *degrees,
        _ => 0.0,
    };
    let tag = item.get("tag");
    let map_id = match tag.and_then(|tag| tag.get("map_uuid")) {
        Some(Value::Long(id)) => Some(*id),
        _ => None,
    };
    let map = map_id.and_then(|id| bot.state().maps.get(id).cloned());
    if let Some(id) = map_id.filter(|_| map.is_none()) {
        if ASKED.with_borrow_mut(|asked| asked.insert(id)) {
            bot.client().send(&MapInfoRequest { map_id: id, client_pixels: Vec::new() });
        }
    }
    let block_id = view.world().registry().states_of(name).next().map_or(0, |(id, _)| id);
    let key = ItemKey { name: name.to_string(), aux, block: block_id, dye: None, banner: None };
    Some(Framed { block, hung: framed::Hung { yaw, tilt }, turn, key, enchanted: tag.is_some_and(|tag| tag.get("ench").is_some()), map })
}

fn picture(image: &Arc<MapImage>) -> Option<Arc<Skin>> {
    PICTURES.with_borrow_mut(|pictures| {
        if let Some((_, skin)) = pictures.iter().find(|(from, _)| Arc::ptr_eq(from, image)) {
            return Some(skin.clone());
        }
        let skin = Arc::new(map::picture(&image.rgba, &crate::held_map::markers(image))?);
        pictures.truncate(PICTURES_KEPT - 1);
        pictures.insert(0, (image.clone(), skin.clone()));
        Some(skin)
    })
}

/// The frames' contents within `reach` blocks of the camera.
pub fn instances(items: &mut Smoother, camera: DVec3, reach: f64) -> Vec<EntityInstance> {
    let frames = SHOWN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let near = frames.iter().filter(|f| (IVec3::from(f.block).as_dvec3() + 0.5).distance_squared(camera) < reach * reach);
    near.filter_map(|f| {
        let block = IVec3::from(f.block);
        if let Some(picture) = f.map.as_ref().and_then(picture) {
            return Some(framed::map_picture(&picture, block, f.hung, f.turn, camera));
        }
        Some(framed::item(&items.item(&f.key, f.enchanted)?, block, f.hung, f.turn, camera))
    })
    .collect()
}
