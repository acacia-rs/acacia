//! Pictures: what `GuiGraphicsExtractor` hands to another renderer. A picture-in-picture
//! (entity, book, banner) is rendered apart and blitted back as an element of the node its rect
//! lands in, ignoring the pose (`PictureInPictureRenderState.bounds`); an atlas sprite blit is an
//! ordinary blit; a map's and a tooltip image's calls come from the source.

use crate::draw::Blend as QuadBlend;

use super::super::gui::IDENTITY;
use super::super::picture::Picture;
use super::Painter;
use super::blit::Prim;
use super::layers::Rect;

impl Painter<'_> {
    pub(super) fn picture(&mut self, p: &Picture) {
        match p {
            Picture::Entity { rect, .. } | Picture::Book { rect, .. } | Picture::Banner { rect, .. } => self.in_picture(p, *rect),
            Picture::SpriteBlit { atlas, sprite, rect, uv } => {
                let Some(s) = self.src.atlas_sprite(atlas, sprite) else { return };
                self.prims(s, &[Prim { rect: *rect, uv: *uv, tile: None }], 0xFFFF_FFFF, QuadBlend::Alpha);
            }
            Picture::Map { .. } | Picture::TooltipImage { .. } => {
                let draws = self.src.picture_draws(p);
                self.draws(&draws);
            }
        }
    }

    fn in_picture(&mut self, p: &Picture, [x0, y0, x1, y1]: [i32; 4]) {
        let rect = Rect::corners(x0, y0, x1, y1);
        let bounds = match self.scissor() {
            Some(s) => s.intersection(rect),
            None => Some(rect),
        };
        if !self.layers.place(bounds) {
            return;
        }
        let window = [x0, y0, x1, y1].map(|v| v as f32 * self.scale);
        let Some(mut q) = self.src.picture(p, window) else { return };
        // The blit is in GUI pixels without the pose; `place_quad` scales and cuts it.
        q.rect = [x0, y0, x1, y1].map(|v| v as f32);
        self.poses.push(IDENTITY);
        let placed = self.place_quad(&q);
        self.poses.pop();
        if let Some(q) = placed {
            self.layers.pictures().push(q);
        }
    }
}
