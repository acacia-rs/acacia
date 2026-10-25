//! Turns `GuiDraw` calls into the quads the game puts on screen, in the order it draws them:
//! sprites through their `.mcmeta` scaling (`blit`), each blit, fill, text and item an element
//! placed by Java's layering (`layers`), poses and scissors applied as `GuiGraphicsExtractor`
//! applies them. Glyphs, item icons and textures come from a [`Source`].

pub mod assets;
pub mod blit;
pub mod layers;
mod picture;

use crate::atlas::Sprite;
use crate::draw::{Blend as QuadBlend, Quad};

use super::gui::{Blend, GuiDraw, IDENTITY, Pose, pose_apply, transform};
use super::picture::Picture;
use super::text::Text;
use blit::{Prim, Scaling};
use layers::{Layers, Rect};

/// Text laid out by `Font.prepareText`: glyph quads in GUI pixels before the pose (shadows first),
/// and the `PreparedTextBuilder` bounds `[left, top, right, bottom]`.
#[derive(Debug, Clone, Default)]
pub struct Glyphs {
    pub quads: Vec<Quad>,
    pub bounds: Option<[f32; 4]>,
}

/// Where the painter finds what the calls name.
pub trait Source {
    /// A GUI sprite (`textures/gui/sprites/<id>`) in the atlas, with its scaling.
    fn sprite(&mut self, id: &str) -> Option<(Sprite, Scaling)>;
    /// A block atlas sprite (`textures/<id>`).
    fn block_sprite(&mut self, id: &str) -> Option<Sprite>;
    /// A whole texture, `path` under `textures/` without `.png`.
    fn texture(&mut self, path: &str) -> Option<Sprite>;
    /// An opaque white texel, for fills.
    fn white(&mut self) -> Sprite;
    fn text(&mut self, text: &Text, x: i32, y: i32, color: u32, shadow: bool) -> Glyphs;
    /// `Font.width`.
    fn width(&mut self, text: &Text) -> i32;
    /// `Util.getMillis()`, which scrolls labels too wide for their box.
    fn millis(&self) -> i64;
    /// The item `key`'s icon over the 16 × 16 GUI pixels at (`x`, `y`), before the pose.
    fn item(&mut self, key: i32, x: i32, y: i32, seed: i32) -> Vec<Quad>;
    /// What `itemDecorations` draws for the item `key` (count, bar, cooldown) as calls.
    fn decorations(&mut self, key: i32, x: i32, y: i32, count: Option<&str>) -> Vec<GuiDraw>;
    /// A sprite of a named atlas (`minecraft:entity/banner/creeper` in `banner_patterns`).
    fn atlas_sprite(&mut self, atlas: &str, sprite: &str) -> Option<Sprite>;
    /// A picture-in-picture (entity, book, banner) over `window` (window pixels): the quad that
    /// shows it there, or `None` when it can't be drawn. See [`Quad::picture`].
    fn picture(&mut self, picture: &Picture, window: [f32; 4]) -> Option<Quad>;
    /// What a map or a tooltip image draws, as calls at its origin.
    fn picture_draws(&mut self, picture: &Picture) -> Vec<GuiDraw>;
}

/// The frame's GUI: quads in window pixels and, when a screen blurs what is under it, how many
/// of them come before the blur.
#[derive(Debug, Default)]
pub struct Painted {
    pub quads: Vec<Quad>,
    pub blur_at: Option<usize>,
}

struct Painter<'a> {
    src: &'a mut dyn Source,
    scale: f32,
    screen: Rect,
    poses: Vec<Pose>,
    scissors: Vec<Rect>,
    layers: Layers,
}

/// `draws` on a `screen`-sized GUI (GUI pixels) at `scale` window pixels per GUI pixel.
pub fn paint(draws: &[GuiDraw], screen: [i32; 2], scale: f32, src: &mut dyn Source) -> Painted {
    let mut p = Painter { src, scale, screen: Rect { x: 0, y: 0, w: screen[0], h: screen[1] }, poses: vec![IDENTITY], scissors: Vec::new(), layers: Layers::default() };
    p.draws(draws);
    let (quads, blur_at) = p.layers.flatten();
    Painted { quads, blur_at }
}

pub fn argb(c: u32) -> [u8; 4] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]
}

fn quad_blend(b: Blend) -> QuadBlend {
    match b {
        Blend::Textured => QuadBlend::Alpha,
        Blend::Vignette => QuadBlend::Darken,
        Blend::Nausea => QuadBlend::Add,
        Blend::Crosshair => QuadBlend::Invert,
    }
}

impl Painter<'_> {
    fn pose(&self) -> Pose {
        *self.poses.last().unwrap_or(&IDENTITY)
    }

    fn scissor(&self) -> Option<Rect> {
        self.scissors.last().copied()
    }

    fn push_scissor(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let r = Rect::corners(x0, y0, x1, y1).axis_aligned(&self.pose());
        let last = self.scissor().unwrap_or(self.screen);
        self.scissors.push(r.intersection(last).unwrap_or(Rect::EMPTY));
    }

    fn draws(&mut self, draws: &[GuiDraw]) {
        for d in draws {
            self.draw(d);
        }
    }

    fn draw(&mut self, d: &GuiDraw) {
        match d {
            GuiDraw::Push(ops) => {
                let mut m = self.pose();
                for op in ops {
                    pose_apply(&mut m, *op);
                }
                self.poses.push(m);
            }
            GuiDraw::Pop => {
                if self.poses.len() > 1 {
                    self.poses.pop();
                }
            }
            GuiDraw::Scissor { x0, y0, x1, y1 } => self.push_scissor(*x0, *y0, *x1, *y1),
            GuiDraw::Unscissor => {
                self.scissors.pop();
            }
            GuiDraw::Stratum => self.layers.next_stratum(),
            GuiDraw::Blur => self.layers.blur(),
            GuiDraw::Sprite { sprite, x, y, w, h, region, color, blend } => {
                let Some((s, scaling)) = self.src.sprite(sprite).or_else(|| self.src.sprite(assets::MISSING)) else { return };
                let mut prims = Vec::new();
                match region {
                    Some([sw, sh, u, v]) if scaling != Scaling::Stretch => {
                        self.push_scissor(*x, *y, x + w, y + h);
                        blit::sprite(scaling, x - u, y - v, *sw, *sh, &mut prims);
                        self.prims(s, &prims, *color, quad_blend(*blend));
                        self.scissors.pop();
                        return;
                    }
                    Some([sw, sh, u, v]) => blit::region([*sw, *sh], [*u, *v], *x, *y, *w, *h, &mut prims),
                    None => blit::sprite(scaling, *x, *y, *w, *h, &mut prims),
                }
                self.prims(s, &prims, *color, quad_blend(*blend));
            }
            GuiDraw::AtlasSprite { sprite, x, y, w, h, color } => {
                let Some(s) = self.src.block_sprite(sprite) else { return };
                let mut prims = Vec::new();
                blit::sprite(Scaling::Stretch, *x, *y, *w, *h, &mut prims);
                self.prims(s, &prims, *color, QuadBlend::Alpha);
            }
            GuiDraw::Blit { texture, blend, x, y, w, h, uv, src, tex, color } => {
                let Some(s) = self.src.texture(texture) else { return };
                let (tw, th) = (tex[0] as f32, tex[1] as f32);
                let uv = [uv[0] / tw, (uv[0] + src[0] as f32) / tw, uv[1] / th, (uv[1] + src[1] as f32) / th];
                self.prims(s, &[Prim { rect: [*x, *y, x + w, y + h], uv, tile: None }], *color, quad_blend(*blend));
            }
            GuiDraw::Fill { x0, y0, x1, y1, color } => self.fill([*x0, *y0, *x1, *y1], *color, *color, QuadBlend::Alpha),
            GuiDraw::Gradient { x0, y0, x1, y1, top, bottom } => self.fill([*x0, *y0, *x1, *y1], *top, *bottom, QuadBlend::Alpha),
            GuiDraw::Highlight { x0, y0, x1, y1, invert } => {
                let rect = [*x0, *y0, *x1, *y1];
                if *invert {
                    self.fill(rect, 0xFFFF_FFFF, 0xFFFF_FFFF, QuadBlend::Invert);
                }
                self.fill(rect, 0xFF00_00FF, 0xFF00_00FF, QuadBlend::Add);
            }
            // GuiGraphicsExtractor.text and the text collectors add nothing without alpha.
            GuiDraw::Text { color, .. } if color >> 24 == 0 => {}
            GuiDraw::Text { text, x, y, color, shadow } => {
                let glyphs = self.src.text(text, *x, *y, *color, *shadow);
                let Some([l, t, r, b]) = glyphs.bounds.filter(|[l, t, r, b]| l < r && t < b) else { return };
                let (pose, scissor) = (self.pose(), self.scissor());
                let bounds = Rect::corners(l.floor() as i32, t.floor() as i32, r.ceil() as i32, b.ceil() as i32).bounds(&pose, scissor);
                if self.layers.place(bounds) {
                    let quads: Vec<Quad> = glyphs.quads.iter().filter_map(|q| self.place_quad(q)).collect();
                    self.layers.glyphs().extend(quads);
                }
            }
            GuiDraw::Item { key, x, y, seed } => {
                let bounds = Rect { x: *x, y: *y, w: 16, h: 16 }.bounds(&self.pose(), self.scissor());
                if self.layers.place(bounds) {
                    let icon = self.src.item(*key, *x, *y, *seed);
                    let quads: Vec<Quad> = icon.iter().filter_map(|q| self.place_quad(q)).collect();
                    self.layers.blits().extend(quads);
                }
            }
            GuiDraw::Decorations { key, x, y, count } => {
                let draws = self.src.decorations(*key, *x, *y, count.as_deref());
                self.draws(&draws);
            }
            GuiDraw::Scrolling { text, center_x, x0, x1, y0, y1, color } => self.scrolling(text, *center_x, [*x0, *y0, *x1, *y1], *color),
            GuiDraw::Picture(p) => self.picture(p),
        }
    }

    /// `ActiveTextCollector.defaultScrollingHelper`: centred in the box, or, when wider, cut to
    /// it and swung from end to end on the clock.
    fn scrolling(&mut self, text: &Text, center_x: i32, [x0, y0, x1, y1]: [i32; 4], color: u32) {
        let width = self.src.width(text);
        let top = (y0 + y1 - 9) / 2 + 1;
        let room = x1 - x0;
        if width > room {
            let max = f64::from(width - room);
            let time = self.src.millis() as f64 / 1000.0;
            let period = (max * 0.5).max(3.0);
            let alpha = (std::f64::consts::FRAC_PI_2 * (std::f64::consts::TAU * time / period).cos()).sin() / 2.0 + 0.5;
            let pos = alpha * max;
            self.push_scissor(x0, y0, x1, y1);
            self.draw(&GuiDraw::Text { text: text.clone(), x: x0 - pos as i32, y: top, color, shadow: true });
            self.scissors.pop();
        } else {
            let (lo, hi) = (x0 + width / 2, x1 - width / 2);
            let x = if center_x < lo { lo } else if center_x > hi { hi } else { center_x };
            self.draw(&GuiDraw::Text { text: text.clone(), x: x - width / 2, y: top, color, shadow: true });
        }
    }

    /// Blits of `sprite`, each its own element.
    fn prims(&mut self, sprite: Sprite, prims: &[Prim], color: u32, blend: QuadBlend) {
        let (pose, scissor) = (self.pose(), self.scissor());
        let texel = |u: f32, v: f32| [sprite.x as f32 + u * sprite.width as f32, sprite.y as f32 + v * sprite.height as f32];
        for p in prims {
            let [x0, y0, x1, y1] = p.rect;
            if !self.layers.place(Rect::corners(x0, y0, x1, y1).bounds(&pose, scissor)) {
                continue;
            }
            for (rect, [u0, u1, v0, v1]) in blit::tiles(p) {
                let ([ul, vt], [ur, vb]) = (texel(u0, v0), texel(u1, v1));
                let q = Quad { blend, ..Quad::plain(rect.map(|v| v as f32), [ul, vt, ur, vb], argb(color)) };
                if let Some(q) = self.place_quad(&q) {
                    self.layers.blits().push(q);
                }
            }
        }
    }

    fn fill(&mut self, [x0, y0, x1, y1]: [i32; 4], top: u32, bottom: u32, blend: QuadBlend) {
        if !self.layers.place(Rect::corners(x0, y0, x1, y1).bounds(&self.pose(), self.scissor())) {
            return;
        }
        let white = self.src.white();
        let (u, v) = (white.x as f32 + 0.5, white.y as f32 + 0.5);
        let q = Quad { bottom: argb(bottom), blend, ..Quad::plain([x0, y0, x1, y1].map(|v| v as f32), [u, v, u, v], argb(top)) };
        if let Some(q) = self.place_quad(&q) {
            self.layers.blits().push(q);
        }
    }

    /// `q` (GUI pixels) through the pose, cut to the scissor, in window pixels.
    fn place_quad(&self, q: &Quad) -> Option<Quad> {
        let pose = self.pose();
        let [l, t, r, b] = q.rect;
        let ([x0, y0], [x1, y1]) = (transform(&pose, l, t), transform(&pose, r, b));
        let [mut ul, mut vt, mut ur, mut vb] = q.uv;
        if x1 < x0 {
            std::mem::swap(&mut ul, &mut ur);
        }
        if y1 < y0 {
            std::mem::swap(&mut vt, &mut vb);
        }
        let rect = [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)];
        let mut out = Quad { rect, uv: [ul, vt, ur, vb], skew: q.skew.map(|s| s * pose[0]), ..*q };
        if let Some(s) = self.scissor() {
            out = cut(out, [s.x as f32, s.y as f32, s.right() as f32, s.bottom() as f32])?;
        }
        out.rect = out.rect.map(|v| v * self.scale);
        out.skew = out.skew.map(|v| v * self.scale);
        Some(out)
    }
}

/// `q` cut to `clip`, its UVs and gradient cut in proportion.
fn cut(q: Quad, clip: [f32; 4]) -> Option<Quad> {
    let [l, t, r, b] = q.rect;
    let (nl, nt, nr, nb) = (l.max(clip[0]), t.max(clip[1]), r.min(clip[2]), b.min(clip[3]));
    if nl >= nr || nt >= nb {
        return None;
    }
    let frac = |from: f32, to: f32, at: f32| if to == from { 0.0 } else { (at - from) / (to - from) };
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let [ul, vt, ur, vb] = q.uv;
    let (ft, fb) = (frac(t, b, nt), frac(t, b, nb));
    let mix = |t: f32| std::array::from_fn(|i| lerp(f32::from(q.color[i]), f32::from(q.bottom[i]), t).round() as u8);
    let uv = [lerp(ul, ur, frac(l, r, nl)), lerp(vt, vb, ft), lerp(ul, ur, frac(l, r, nr)), lerp(vt, vb, fb)];
    Some(Quad { rect: [nl, nt, nr, nb], uv, color: mix(ft), bottom: mix(fb), ..q })
}

#[cfg(test)]
mod tests;
