//! The loading screens: `LevelLoadingScreen` (the terrain text, the server's chunk map and the
//! progress bar over a portal or menu background) and `GenericMessageScreen` (one boxed line).

use super::{Env, Escape, Screen, arena_children, blurred, menu_texture, panorama, render_all};
use crate::java::gui::GuiDraw;
use crate::java::picture::Picture;
use crate::java::text::Text;
use crate::java::widget::Ctx;
use crate::java::widget::input::Child;
use crate::java::widget::layout::Arena;
use crate::java::widget::text::{Fill, FocusableTextWidget};

/// `LevelLoadingScreen.Reason`: what the screen is drawn over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    NetherPortal,
    EndPortal,
    Other,
}

/// `ChunkStatus`es in `COLORS`' order; `None` in a cell is a chunk with no status yet.
pub const STATUSES: [&str; 10] =
    ["empty", "structure_starts", "structure_references", "biomes", "terrain", "features", "initialize_light", "light", "spawn", "full"];

const COLORS: [u32; 10] = [5_526_612, 10_066_329, 6_250_897, 8_434_258, 3_159_410, 2_213_376, 13_421_772, 16_769_184, 15_884_384, 16_777_215];

/// `ChunkLoadStatusView`: `radius` and the statuses of the `2 * radius + 1` square, x major.
#[derive(Clone, Debug, PartialEq)]
pub struct ChunkView {
    pub radius: i32,
    pub cells: Vec<Option<usize>>,
}

impl ChunkView {
    fn get(&self, x: i32, z: i32) -> Option<usize> {
        let d = self.radius * 2 + 1;
        self.cells.get((x * d + z) as usize).copied().flatten()
    }
}

/// What `LevelLoadTracker` tells the screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Load {
    /// `hasProgress`: the server reported a stage.
    pub has_progress: bool,
    /// `serverProgress`, 0 to 1.
    pub progress: f32,
    pub view: Option<ChunkView>,
    /// The `visualize_chunks_on_server` debug entry: a red mark under the centre chunk.
    pub visualize: bool,
}

pub struct LevelLoadingScreen {
    pub env: Env,
    pub reason: Reason,
    pub load: Load,
    /// `smoothedProgress`, eased toward the server's progress each tick.
    pub smoothed: f32,
}

impl LevelLoadingScreen {
    pub fn new(env: Env, reason: Reason, load: Load) -> LevelLoadingScreen {
        LevelLoadingScreen { env, reason, load, smoothed: 0.0 }
    }

    /// `tick` (closing once the level is ready is the owner's).
    pub fn tick(&mut self) {
        self.smoothed += (self.load.progress - self.smoothed) * 0.2;
    }
}

/// `extractChunksForRendering(graphics, xCenter, yCenter, size, margin, view)`.
pub fn chunks(ctx: &mut Ctx, x_center: i32, y_center: i32, size: i32, margin: i32, view: &ChunkView, visualize: bool) {
    let width = size + margin;
    let diameter = view.radius * 2 + 1;
    let total = diameter * width - margin;
    let (x0, y0) = (x_center - total / 2, y_center - total / 2);
    if visualize {
        let c = width / 2 + 1;
        ctx.gui.fill(x_center - c, y_center - c, x_center + c, y_center + c, 0xFFFF_0000);
    }
    for x in 0..diameter {
        for z in 0..diameter {
            let color = view.get(x, z).map_or(0, |s| COLORS[s]);
            let (cx, cy) = (x0 + x * width, y0 + z * width);
            ctx.gui.fill(cx, cy, cx + size, cy + size, color | 0xFF00_0000);
        }
    }
}

impl Screen for LevelLoadingScreen {
    fn background(&mut self, ctx: &mut Ctx) {
        let (w, h) = (self.env.width, self.env.height);
        match self.reason {
            Reason::NetherPortal => {
                ctx.gui.draws.push(GuiDraw::AtlasSprite { sprite: "block/nether_portal".into(), x: 0, y: 0, w, h, color: 0xFFFF_FFFF });
            }
            Reason::EndPortal => ctx.gui.draws.push(GuiDraw::Picture(Picture::EndPortal { rect: [0, 0, w, h] })),
            Reason::Other => {
                panorama(&self.env, ctx);
                blurred(&self.env, ctx);
                menu_texture(&self.env, ctx, 0, 0, w, h);
            }
        }
    }

    fn foreground(&mut self, ctx: &mut Ctx) {
        let (xc, yc) = (self.env.width / 2, self.env.height / 2);
        let top = match &self.load.view {
            Some(view) => {
                chunks(ctx, xc, yc, 2, 0, view, self.load.visualize);
                yc - view.radius * 2 - 9 * 3
            }
            None => yc - 50,
        };
        ctx.centered_text(ctx.tr("multiplayer.downloadingTerrain"), xc, top, 0xFFFF_FFFF);
        if self.load.has_progress {
            let (left, y) = (xc - 100, top + 9 + 3);
            ctx.gui.fill(left, y, left + 200, y + 2, 0xFF00_0000);
            ctx.gui.fill(left, y, left + (self.smoothed * 200.0).round() as i32, y + 2, 0xFF00_FF00);
        }
    }

    fn escape(&self) -> Escape {
        Escape::Ignore
    }
}

/// `GenericMessageScreen`: the title in a 12-padded box at the centre.
pub struct GenericMessageScreen {
    pub env: Env,
    widgets: Arena,
}

impl GenericMessageScreen {
    pub fn new(ctx: &Ctx, env: Env, title: Text) -> GenericMessageScreen {
        let max = ctx.width(&title) + 24;
        let mut box_ = FocusableTextWidget::new(ctx.font, title, 12, max, true, Fill::Always);
        box_.base.x = env.width / 2 - box_.base.w / 2;
        box_.base.y = env.height / 2 - 9 / 2;
        GenericMessageScreen { env, widgets: vec![Box::new(box_)] }
    }
}

impl Screen for GenericMessageScreen {
    fn background(&mut self, ctx: &mut Ctx) {
        panorama(&self.env, ctx);
        blurred(&self.env, ctx);
        menu_texture(&self.env, ctx, 0, 0, self.env.width, self.env.height);
    }

    fn foreground(&mut self, ctx: &mut Ctx) {
        render_all(&mut self.widgets, ctx);
    }

    fn children(&mut self) -> Vec<Child<'_>> {
        arena_children(&mut self.widgets)
    }

    fn escape(&self) -> Escape {
        Escape::Ignore
    }
}
