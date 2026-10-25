use super::*;
use crate::java::gui::{Gui, PoseOp};
use crate::java::picture::Picture;

struct Fake;

const WHITE_TEXEL: Sprite = Sprite { x: 0, y: 0, width: 1, height: 1 };

impl Source for Fake {
    fn sprite(&mut self, id: &str) -> Option<(Sprite, Scaling)> {
        match id {
            "widget/button" => Some((Sprite { x: 0, y: 100, width: 200, height: 20 }, Scaling::NineSlice { w: 200, h: 20, border: [3; 4], stretch_inner: false })),
            "hud/heart" => Some((Sprite { x: 10, y: 0, width: 9, height: 9 }, Scaling::Stretch)),
            "missingno" => Some((Sprite { x: 30, y: 0, width: 16, height: 16 }, Scaling::Stretch)),
            _ => None,
        }
    }
    fn block_sprite(&mut self, _: &str) -> Option<Sprite> {
        None
    }
    fn texture(&mut self, _: &str) -> Option<Sprite> {
        Some(Sprite { x: 0, y: 200, width: 256, height: 256 })
    }
    fn white(&mut self) -> Sprite {
        WHITE_TEXEL
    }
    fn text(&mut self, _: &Text, x: i32, y: i32, color: u32, _: bool) -> Glyphs {
        let (x, y) = (x as f32, y as f32);
        Glyphs { quads: vec![Quad::plain([x, y, x + 5.0, y + 8.0], [0.0; 4], argb(color))], bounds: Some([x, y, x + 6.0, y + 9.0]) }
    }
    fn width(&mut self, text: &Text) -> i32 {
        6 * text.plain().chars().count() as i32
    }
    fn millis(&self) -> i64 {
        0
    }
    fn item(&mut self, _: i32, x: i32, y: i32, _: i32) -> Vec<Quad> {
        let (x, y) = (x as f32, y as f32);
        vec![Quad::plain([x, y, x + 16.0, y + 16.0], [0.0; 4], [255; 4])]
    }
    fn decorations(&mut self, _: i32, x: i32, y: i32, _: Option<&str>) -> Vec<GuiDraw> {
        vec![GuiDraw::Fill { x0: x, y0: y, x1: x + 1, y1: y + 1, color: 0xFF00_FF00 }]
    }
    fn atlas_sprite(&mut self, _: &str, _: &str) -> Option<Sprite> {
        Some(Sprite { x: 64, y: 0, width: 64, height: 64 })
    }
    fn picture(&mut self, _: &Picture, window: [f32; 4]) -> Option<Quad> {
        Some(Quad::picture(window))
    }
    fn picture_draws(&mut self, _: &Picture) -> Vec<GuiDraw> {
        Vec::new()
    }
}

#[test]
fn a_picture_ignores_the_pose_and_follows_its_nodes_blits() {
    let entity = Picture::Entity { entity: "player".into(), scale: 30.0, translation: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0], camera: None, angles: None, rect: [10, 10, 40, 50] };
    let mut gui = Gui::new(100, 100);
    gui.push(vec![PoseOp::Translate(50.0, 0.0)]);
    gui.draws.push(GuiDraw::Picture(entity));
    gui.sprite("hud/heart", 0, 0, 9, 9);
    gui.pop();
    let painted = paint(&gui.draws, [100, 100], 2.0, &mut Fake);
    // The heart, translated away from the picture, shares its node and is drawn first.
    assert_eq!(painted.quads[0].rect, [100.0, 0.0, 118.0, 18.0]);
    let p = painted.quads[1];
    assert!(p.picture);
    assert_eq!((p.rect, p.uv), ([20.0, 20.0, 80.0, 100.0], [20.0, 20.0, 80.0, 100.0]));
}

#[test]
fn a_banner_pattern_blit_takes_its_part_of_the_atlas_sprite() {
    let blit = Picture::SpriteBlit { atlas: "banner_patterns".into(), sprite: "minecraft:entity/banner/base".into(), rect: [0, 0, 5, 10], uv: [0.0, 0.5, 0.0, 0.25] };
    let painted = paint(&[GuiDraw::Picture(blit)], [100, 100], 1.0, &mut Fake);
    assert_eq!((painted.quads[0].rect, painted.quads[0].uv), ([0.0, 0.0, 5.0, 10.0], [64.0, 0.0, 96.0, 16.0]));
}

#[test]
fn a_texture_blit_past_its_image_wraps_within_its_sprite() {
    let blit = |w: i32| GuiDraw::Blit { texture: "gui/menu_background".into(), blend: Blend::Textured, x: 0, y: 0, w, h: 32, uv: [0.0, 0.0], src: [w, 32], tex: [256, 256], color: 0xFFFF_FFFF };
    let painted = paint(&[blit(512), blit(256)], [600, 100], 1.0, &mut Fake);
    let ([wide, fits], sprite) = ([painted.quads[0], painted.quads[1]], [0.0, 200.0, 256.0, 456.0]);
    assert_eq!((wide.uv, wide.wrap), ([0.0, 200.0, 512.0, 232.0], Some(sprite)));
    assert_eq!(fits.wrap, None);
}

#[test]
fn sprites_scale_and_text_over_a_sprite_comes_after_it() {
    let mut gui = Gui::new(100, 100);
    gui.text(Text::literal("a"), 0, 0, 0xFFFF_FFFF, false);
    gui.sprite("hud/heart", 0, 0, 9, 9);
    let painted = paint(&gui.draws, [100, 100], 2.0, &mut Fake);
    assert_eq!(painted.quads.len(), 2);
    assert_eq!(painted.quads[0].rect, [0.0, 0.0, 10.0, 16.0], "the heart overlaps the text: a node above it");
    assert_eq!(painted.quads[1].rect, [0.0, 0.0, 18.0, 18.0]);
    assert_eq!(painted.quads[1].uv, [10.0, 0.0, 19.0, 9.0]);
}

#[test]
fn a_wide_button_is_nine_elements_and_its_middle_tiles() {
    let mut gui = Gui::new(500, 100);
    gui.sprite("widget/button", 0, 0, 400, 20);
    let painted = paint(&gui.draws, [500, 100], 1.0, &mut Fake);
    // At the sprite's height: left cap, tiled middle (two tiles of 194, then 12), right cap.
    assert_eq!(painted.quads.len(), 5);
    assert_eq!(painted.quads[3].rect, [391.0, 0.0, 397.0, 20.0]);
    let uv = painted.quads[3].uv;
    assert!(uv.iter().zip([3.0, 100.0, 9.0, 120.0]).all(|(a, b)| (a - b).abs() < 1e-3), "{uv:?}");
}

#[test]
fn poses_scissors_and_gradients() {
    let mut gui = Gui::new(100, 100);
    gui.push(vec![PoseOp::Translate(10.0, 10.0), PoseOp::Scale(2.0, 2.0)]);
    gui.item(0, 0, 0, 0);
    gui.decorations(0, 0, 0, None);
    gui.pop();
    gui.scissor(0, 0, 50, 5);
    gui.gradient(0, 0, 10, 10, 0xFF00_0000, 0xFFFF_FFFF);
    gui.unscissor();
    let painted = paint(&gui.draws, [100, 100], 1.0, &mut Fake);
    assert_eq!(painted.quads[0].rect, [10.0, 10.0, 42.0, 42.0]);
    assert_eq!(painted.quads[2].rect, [10.0, 10.0, 12.0, 12.0], "the icon covers the count fill: a node above");
    // Overlapping neither, the gradient goes back to the bottom node, before the fill.
    let g = painted.quads[1];
    assert_eq!(g.rect, [0.0, 0.0, 10.0, 5.0]);
    assert_eq!((g.color, g.bottom), ([0, 0, 0, 255], [128, 128, 128, 255]), "cut in half, the gradient with it");
}

#[test]
fn a_label_is_centred_in_its_box_or_scrolled_and_cut_to_it() {
    let label = |text: &str, x1| GuiDraw::Scrolling { text: Text::literal(text), center_x: 50, x0: 0, x1, y0: 0, y1: 20, color: 0xFFFF_FFFF };
    let painted = paint(&[label("Done", 100)], [200, 100], 1.0, &mut Fake);
    assert_eq!(painted.quads[0].rect, [38.0, 6.0, 43.0, 14.0], "24 wide about 50, at (20 - 9) / 2 + 1");
    // At millis 0 the swing is at its far end, 14 px left: the fake's one glyph falls outside the box.
    let painted = paint(&[label("Done", 10)], [200, 100], 1.0, &mut Fake);
    assert!(painted.quads.is_empty());
}

#[test]
fn a_sprite_the_atlas_lacks_draws_the_missing_one() {
    let mut gui = Gui::new(100, 100);
    gui.sprite("tooltip/fancy/background", 0, 0, 40, 20);
    let painted = paint(&gui.draws, [100, 100], 1.0, &mut Fake);
    assert_eq!((painted.quads[0].rect, painted.quads[0].uv), ([0.0, 0.0, 40.0, 20.0], [30.0, 0.0, 46.0, 16.0]));
}

#[test]
fn a_fully_faded_label_adds_nothing() {
    let label = GuiDraw::Scrolling { text: Text::literal("Done"), center_x: 50, x0: 0, x1: 100, y0: 0, y1: 20, color: 0x00FF_FFFF };
    let painted = paint(&[label, GuiDraw::Fill { x0: 30, y0: 0, x1: 70, y1: 20, color: 0xFF00_0000 }], [200, 100], 1.0, &mut Fake);
    assert_eq!(painted.quads.len(), 1, "no glyphs, and no node taken by them");
}
