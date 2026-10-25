//! A Java look's items (`java/item_models.json`, `java/item_names.json`, written by `lookbake
//! java`): per Java item and display context, the quads the game resolves for its default stack and
//! the display transform. See README "Java items".

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use serde::Deserialize;

use super::hand::Display;
use crate::entity::Skin;
use crate::entity::bake::Vertex;

pub const MODELS: &str = "java/item_models.json";
pub const NAMES: &str = "java/item_names.json";

/// `ItemDisplayContext`'s serialized names.
pub mod context {
    pub const GUI: &str = "gui";
    pub const GROUND: &str = "ground";
    pub const FIXED: &str = "fixed";
    pub const HEAD: &str = "head";

    pub fn hand(first_person: bool, left: bool) -> &'static str {
        match (first_person, left) {
            (false, false) => "thirdperson_righthand",
            (false, true) => "thirdperson_lefthand",
            (true, false) => "firstperson_righthand",
            (true, true) => "firstperson_lefthand",
        }
    }

    pub fn left_hand(context: &str) -> bool {
        context.ends_with("_lefthand")
    }
}

/// In the layer's space: the item's unit box, before the display's `-0.5`.
#[derive(Deserialize)]
pub struct Quad {
    /// `minecraft:textures/item/apple.png`.
    pub image: String,
    pub pos: [f32; 12],
    /// Within the image's first frame, 0 to 1.
    pub uv: [f32; 8],
    pub normal: [f32; 3],
    /// Index into the layer's tints; -1 for none.
    pub tint: i32,
    /// ARGB, multiplied in: a special model's colour.
    pub color: i32,
}

#[derive(Deserialize)]
struct Geometry {
    quads: Vec<Quad>,
}

#[derive(Deserialize)]
pub struct Layer {
    mesh: usize,
    /// ARGB, by quad tint index.
    pub tints: Vec<i32>,
    /// `NONE`, `STANDARD` or `SPECIAL`.
    pub foil: String,
    /// Rotation in degrees, translation in blocks, scale: `ItemTransform`'s.
    pub transform: [f32; 9],
    /// Lit as a block in the GUI (ITEMS_3D), else as a flat item.
    pub block_light: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Entry {
    Contexts(HashMap<String, Vec<Layer>>),
    #[allow(dead_code)]
    Failed { error: String },
}

#[derive(Deserialize)]
struct File {
    meshes: Vec<Geometry>,
    items: HashMap<String, Entry>,
}

pub struct JavaItems {
    root: PathBuf,
    meshes: Vec<Geometry>,
    items: HashMap<String, HashMap<String, Vec<Layer>>>,
    /// Bedrock item to Java item, by aux (`""` for any).
    names: HashMap<String, HashMap<String, String>>,
}

impl JavaItems {
    /// The look's items, parsed once per look directory while anything holds them; `None` for a
    /// look without them.
    pub fn shared(root: &Path) -> Option<Arc<JavaItems>> {
        static LOADED: Mutex<Vec<(PathBuf, Weak<JavaItems>)>> = Mutex::new(Vec::new());
        let mut loaded = LOADED.lock().unwrap_or_else(|e| e.into_inner());
        loaded.retain(|(_, items)| items.strong_count() > 0);
        if let Some(items) = loaded.iter().find(|(path, _)| path == root).and_then(|(_, items)| items.upgrade()) {
            return Some(items);
        }
        let items = Arc::new(JavaItems::load(root)?);
        loaded.push((root.to_owned(), Arc::downgrade(&items)));
        Some(items)
    }

    pub fn load(root: &Path) -> Option<JavaItems> {
        let file: File = serde_json::from_slice(&std::fs::read(root.join(MODELS)).ok()?)
            .map_err(|error| tracing::warn!(%error, "unreadable {MODELS}"))
            .ok()?;
        let items = file.items.into_iter().filter_map(|(id, entry)| match entry {
            Entry::Contexts(contexts) => Some((id, contexts)),
            Entry::Failed { .. } => None,
        });
        let names = std::fs::read(root.join(NAMES)).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        Some(JavaItems { root: root.to_owned(), meshes: file.meshes, items: items.collect(), names })
    }

    /// The Java item a Bedrock one is: from the table, else the same name.
    pub fn java_name<'a>(&'a self, bedrock: &'a str, aux: u32) -> &'a str {
        let by_aux = self.names.get(bedrock);
        by_aux.and_then(|by| by.get(&aux.to_string()).or_else(|| by.get(""))).map_or(bedrock, String::as_str)
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.items.keys().map(String::as_str)
    }

    pub fn layers(&self, item: &str, context: &str) -> Option<&[Layer]> {
        self.items.get(item)?.get(context).map(Vec::as_slice)
    }

    pub fn quads(&self, layer: &Layer) -> &[Quad] {
        self.meshes.get(layer.mesh).map_or(&[], |m| m.quads.as_slice())
    }

    /// `context`'s display, from its first layer; a left hand's mirrored as `ItemTransform.apply` does.
    pub fn display(&self, item: &str, context: &str) -> Option<Display> {
        Some(Display::java(self.layers(item, context)?.first()?.transform, context::left_hand(context)))
    }

    pub fn block_light(&self, item: &str, context: &str) -> Option<bool> {
        Some(self.layers(item, context)?.first()?.block_light)
    }

    /// The display per context the hands, the ground and frames use.
    pub fn displays(&self, item: &str) -> Displays {
        let contexts = self.items.get(item).into_iter().flatten();
        Displays(contexts.filter_map(|(context, layers)| Some((context.clone(), Display::java(layers.first()?.transform, context::left_hand(context))))).collect())
    }

    /// `context`'s layers as one textured mesh; `dye` replaces the first tint (dyed leather).
    pub fn mesh(&self, item: &str, context: &str, dye: Option<[u8; 3]>) -> Option<ItemMesh> {
        super::java_mesh::build(self, self.layers(item, context)?, dye)
    }

    pub(super) fn image_path(&self, image: &str) -> PathBuf {
        self.root.join("java").join(image.strip_prefix("minecraft:").unwrap_or(image))
    }
}

/// An item's displays by context name.
#[derive(Debug, Default)]
pub struct Displays(HashMap<String, Display>);

impl Displays {
    pub fn get(&self, context: &str) -> Option<Display> {
        self.0.get(context).copied()
    }
}

/// [`JavaItems::mesh`]: the skin, and where each quad's texture went, for checking it against the game.
pub struct ItemMesh {
    pub skin: Skin,
    pub regions: Vec<Region>,
    /// Each quad's region; the mesh holds six vertices per quad (corners 0, 1, 2, 0, 2, 3).
    pub quad_regions: Vec<usize>,
    /// Some layer shimmers.
    pub foil: bool,
    /// Deeper than an extruded sprite.
    pub solid: bool,
}

/// One image, tinted, at `y` in the skin.
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    pub image: String,
    pub argb: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    /// The skin's texture coordinates back to the image's.
    pub fn sprite_uv(&self, skin: &Skin, [u, v]: [f32; 2]) -> [f32; 2] {
        [u * skin.width as f32 / self.width as f32, (v * skin.height as f32 - self.y as f32) / self.height as f32]
    }
}

pub(super) fn vertex(quad: &Quad, corner: usize, uv: [f32; 2]) -> Vertex {
    let p = &quad.pos[corner * 3..corner * 3 + 3];
    // Centred on the item box, z mirrored (meshes face -z); see hand::MESH.
    Vertex { position: [p[0] - 0.5, p[1] - 0.5, 0.5 - p[2]], bone: 0, normal: [quad.normal[0], quad.normal[1], -quad.normal[2]], uv }
}
