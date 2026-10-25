//! Java's `GuiRenderState` layering, which decides the order things reach the screen: an element
//! goes one node above the highest node holding anything it overlaps (or above the last element
//! when that one covers it), and each stratum starts a fresh tree drawn over all before it. Within
//! a node, blits come before glyphs. Elements without bounds (off-screen, scissored away) are
//! dropped.

use crate::draw::Quad;

use super::super::gui::{Pose, transform};
use super::super::mth::{ceil, floor};

/// A `ScreenRectangle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const EMPTY: Rect = Rect { x: 0, y: 0, w: 0, h: 0 };

    pub fn corners(x0: i32, y0: i32, x1: i32, y1: i32) -> Rect {
        Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }

    pub fn right(self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(self) -> i32 {
        self.y + self.h
    }

    pub fn intersection(self, o: Rect) -> Option<Rect> {
        let (l, t) = (self.x.max(o.x), self.y.max(o.y));
        let (r, b) = (self.right().min(o.right()), self.bottom().min(o.bottom()));
        (l < r && t < b).then(|| Rect::corners(l, t, r, b))
    }

    pub fn intersects(self, o: Rect) -> bool {
        self.x < o.right() && self.right() > o.x && self.y < o.bottom() && self.bottom() > o.y
    }

    pub fn encompasses(self, o: Rect) -> bool {
        o.x >= self.x && o.y >= self.y && o.right() <= self.right() && o.bottom() <= self.bottom()
    }

    /// `transformMaxBounds`: the integer box around the transformed corners.
    pub fn max_bounds(self, m: &Pose) -> Rect {
        let pts = [(self.x, self.y), (self.right(), self.y), (self.x, self.bottom()), (self.right(), self.bottom())].map(|(x, y)| transform(m, x as f32, y as f32));
        let (min_x, max_x) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
        let (min_y, max_y) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
        Rect { x: floor(min_x), y: floor(min_y), w: ceil(max_x - min_x), h: ceil(max_y - min_y) }
    }

    /// `transformAxisAligned`, what a scissor goes through.
    pub fn axis_aligned(self, m: &Pose) -> Rect {
        let a = transform(m, self.x as f32, self.y as f32);
        let b = transform(m, self.right() as f32, self.bottom() as f32);
        Rect { x: floor(a[0]), y: floor(a[1]), w: floor(b[0] - a[0]), h: floor(b[1] - a[1]) }
    }

    /// An element's bounds: `self` through `pose`, cut to `scissor`.
    pub fn bounds(self, pose: &Pose, scissor: Option<Rect>) -> Option<Rect> {
        let b = self.max_bounds(pose);
        match scissor {
            Some(s) => s.intersection(b),
            None => Some(b),
        }
    }
}

#[derive(Default)]
struct Node {
    parent: Option<usize>,
    up: Option<usize>,
    bounds: Vec<Rect>,
    blits: Vec<Quad>,
    glyphs: Vec<Quad>,
}

/// Quads sorted into Java's nodes as their elements arrive.
pub struct Layers {
    nodes: Vec<Node>,
    strata: Vec<usize>,
    current: usize,
    last: Option<Rect>,
    blur: Option<usize>,
}

impl Default for Layers {
    fn default() -> Self {
        Layers { nodes: vec![Node::default()], strata: vec![0], current: 0, last: None, blur: None }
    }
}

impl Layers {
    pub fn next_stratum(&mut self) {
        self.nodes.push(Node::default());
        self.current = self.nodes.len() - 1;
        self.strata.push(self.current);
    }

    /// `blurBeforeThisStratum`: the strata before the current one are blurred.
    pub fn blur(&mut self) {
        self.blur.get_or_insert(self.strata.len() - 1);
    }

    /// `findAppropriateNode`: makes the node an element with `bounds` belongs in current; `false`
    /// (the element is dropped) when it has none.
    pub fn place(&mut self, bounds: Option<Rect>) -> bool {
        let Some(bounds) = bounds else { return false };
        if self.last.is_some_and(|l| l.encompasses(bounds)) {
            self.up();
        } else {
            let mut node = *self.strata.last().unwrap_or(&0);
            while let Some(up) = self.nodes[node].up {
                node = up;
            }
            let found = loop {
                let found = self.nodes[node].bounds.iter().any(|b| b.intersects(bounds));
                match self.nodes[node].parent {
                    Some(parent) if !found => node = parent,
                    _ => break found,
                }
            };
            self.current = node;
            if found {
                self.up();
            }
        }
        self.last = Some(bounds);
        self.nodes[self.current].bounds.push(bounds);
        true
    }

    fn up(&mut self) {
        let up = match self.nodes[self.current].up {
            Some(up) => up,
            None => {
                self.nodes.push(Node { parent: Some(self.current), ..Node::default() });
                let up = self.nodes.len() - 1;
                self.nodes[self.current].up = Some(up);
                up
            }
        };
        self.current = up;
    }

    /// The current node's blits (sprites, fills, items).
    pub fn blits(&mut self) -> &mut Vec<Quad> {
        &mut self.nodes[self.current].blits
    }

    /// The current node's glyphs, drawn after its blits.
    pub fn glyphs(&mut self) -> &mut Vec<Quad> {
        &mut self.nodes[self.current].glyphs
    }

    /// Every quad in drawing order, and how many of them come before the blur.
    pub fn flatten(mut self) -> (Vec<Quad>, Option<usize>) {
        let mut out = Vec::new();
        let mut blur_at = None;
        for (i, &root) in self.strata.iter().enumerate() {
            if self.blur == Some(i) {
                blur_at = Some(out.len());
            }
            let mut node = Some(root);
            while let Some(n) = node {
                let n = &mut self.nodes[n];
                out.append(&mut n.blits);
                out.append(&mut n.glyphs);
                node = n.up;
            }
        }
        (out, blur_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java::gui::IDENTITY;

    fn tagged(layers: &mut Layers, rect: Rect, tag: f32) {
        assert!(layers.place(Some(rect)));
        layers.blits().push(Quad::plain([tag; 4], [0.0; 4], [255; 4]));
    }

    fn order(layers: Layers) -> Vec<f32> {
        layers.flatten().0.iter().map(|q| q.rect[0]).collect()
    }

    #[test]
    fn overlapping_goes_above_and_apart_stays_below() {
        let mut layers = Layers::default();
        tagged(&mut layers, Rect::corners(0, 0, 10, 10), 1.0);
        tagged(&mut layers, Rect::corners(5, 5, 15, 15), 2.0);
        // Touches nothing: back in the bottom node, so drawn before 2 though added after it.
        tagged(&mut layers, Rect::corners(20, 0, 30, 10), 3.0);
        assert_eq!(order(layers), [1.0, 3.0, 2.0]);
    }

    #[test]
    fn glyphs_follow_their_nodes_blits_and_strata_follow_everything() {
        let mut layers = Layers::default();
        assert!(layers.place(Some(Rect::corners(0, 0, 10, 10))));
        layers.glyphs().push(Quad::plain([1.0; 4], [0.0; 4], [255; 4]));
        tagged(&mut layers, Rect::corners(20, 0, 30, 10), 2.0);
        layers.next_stratum();
        layers.blur();
        tagged(&mut layers, Rect::corners(40, 0, 50, 10), 3.0);
        let (quads, blur) = layers.flatten();
        assert_eq!(quads.iter().map(|q| q.rect[0]).collect::<Vec<_>>(), [2.0, 1.0, 3.0]);
        assert_eq!(blur, Some(2));
    }

    #[test]
    fn bounds_round_outwards_and_scissors_cut_them() {
        let pose = [1.5, 0.0, 0.0, 1.5, 0.25, 0.0];
        assert_eq!(Rect::corners(0, 0, 3, 3).max_bounds(&pose), Rect { x: 0, y: 0, w: 5, h: 5 });
        assert_eq!(Rect::corners(0, 0, 3, 3).bounds(&IDENTITY, Some(Rect::corners(5, 5, 9, 9))), None);
        assert_eq!(Rect::corners(0, 0, 3, 3).axis_aligned(&pose), Rect { x: 0, y: 0, w: 4, h: 4 });
    }
}
