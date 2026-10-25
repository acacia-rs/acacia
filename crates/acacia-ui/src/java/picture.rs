//! What a GUI hands to another renderer instead of drawing it: pictures-in-picture (an entity, the
//! enchanting table's book, a banner, a map) and tooltip images (a bundle's contents), by the
//! parameters the game passes. The parity oracle records the same calls as markers.

use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq)]
pub enum Picture {
    /// `entity(state, scale, translation, rotation, cameraAngle, x0, y0, x1, y1)`; `angles` are a
    /// living entity's `bodyRot`, `yRot`, `xRot`.
    Entity { entity: String, scale: f32, translation: [f32; 3], rotation: [f32; 4], camera: Option<[f32; 4]>, angles: Option<[f32; 3]>, rect: [i32; 4] },
    /// `book(model, texture, scale, open, flip, x0, y0, x1, y1)`; `texture` is the path.
    Book { texture: String, scale: f32, open: f32, flip: f32, rect: [i32; 4] },
    /// `bannerPattern(flag, base, layers, x0, y0, x1, y1)`; layers as `"<pattern id> <dye>"`.
    Banner { base: String, patterns: Vec<String>, rect: [i32; 4] },
    /// `map(state)`, drawn at the pose's origin, 128 pixels square.
    Map { decorations: usize },
    /// A non-text `ClientTooltipComponent`'s `extractImage` at `(x, y)`, `w` × `h` big; `kind` is
    /// its Java class's simple name (`ClientBundleTooltip`).
    TooltipImage { kind: String, x: i32, y: i32, w: i32, h: i32 },
    /// `blit(atlas, x0, y0, x1, y1, u0, u1, v0, v1)` of an atlas sprite (`getSprite(spriteId)`), the
    /// UVs within the sprite (0 to 1 across it) since the atlas's layout is the renderer's.
    SpriteBlit { atlas: String, sprite: String, rect: [i32; 4], uv: [f32; 4] },
    /// `fill(END_PORTAL, sky + portal textures, x0, y0, x1, y1)`: the end portal shader over a rect.
    EndPortal { rect: [i32; 4] },
}

fn rect(v: &mut Value, r: &[i32; 4]) {
    v["x0"] = json!(r[0]);
    v["y0"] = json!(r[1]);
    v["x1"] = json!(r[2]);
    v["y1"] = json!(r[3]);
}

impl Picture {
    /// The oracle's marker object (without its pose).
    pub fn encode(&self) -> Value {
        match self {
            Picture::Entity { entity, scale, translation, rotation, camera, angles, rect: r } => {
                let mut v = json!({"type": "entity", "entity": entity, "scale": scale, "translation": translation, "rotation": rotation, "camera": camera});
                if let Some(a) = angles {
                    v["angles"] = json!(a);
                }
                rect(&mut v, r);
                v
            }
            Picture::Book { texture, scale, open, flip, rect: r } => {
                let mut v = json!({"type": "book", "texture": texture, "book": [scale, open, flip]});
                rect(&mut v, r);
                v
            }
            Picture::Banner { base, patterns, rect: r } => {
                let mut v = json!({"type": "banner", "base": base, "patterns": patterns});
                rect(&mut v, r);
                v
            }
            Picture::Map { decorations } => json!({"type": "map", "decorations": decorations}),
            Picture::TooltipImage { kind, x, y, w, h } => json!({"type": "tooltip_image", "kind": kind, "x": x, "y": y, "w": w, "h": h}),
            Picture::SpriteBlit { atlas, sprite, rect: r, uv } => {
                let mut v = json!({"type": "sprite_blit", "atlas": atlas, "sprite": sprite, "uv": uv});
                rect(&mut v, r);
                v
            }
            Picture::EndPortal { rect: r } => {
                let mut v = json!({"type": "end_portal"});
                rect(&mut v, r);
                v
            }
        }
    }
}
