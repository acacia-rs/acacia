//! `DeathScreen`: the red wash, the doubled title, the cause and score, and the two buttons that
//! stay disabled for the first 20 ticks.

use super::{Env, Screen, render_all};
use crate::java::gui::PoseOp;
use crate::java::text::Text;
use crate::java::widget::Ctx;
use crate::java::widget::button::Button;
use crate::java::widget::layout::Arena;

pub struct DeathScreen {
    pub env: Env,
    title: Text,
    cause: Option<Text>,
    score: Text,
    widgets: Arena,
}

impl DeathScreen {
    /// `new DeathScreen(cause, hardcore, player)` after `init` and `ticks` ticks.
    pub fn new(ctx: &Ctx, env: Env, cause: Option<Text>, hardcore: bool, score: i32, ticks: u32) -> DeathScreen {
        let title = ctx.tr(if hardcore { "deathScreen.title.hardcore" } else { "deathScreen.title" });
        let score = ctx.tr_args("deathScreen.score.value", &[Text::literal(score.to_string()).with_color(0xFF_FF55)]);
        let (x, y) = (env.width / 2 - 100, env.height / 4);
        let respawn = ctx.tr(if hardcore { "deathScreen.spectate" } else { "deathScreen.respawn" });
        let mut widgets: Arena = vec![
            Box::new(Button::new(x, y + 72, 200, 20, respawn)),
            Box::new(Button::new(x, y + 96, 200, 20, ctx.tr("deathScreen.titleScreen"))),
        ];
        for w in &mut widgets {
            w.base_mut().active = ticks >= 20;
        }
        DeathScreen { env, title, cause, score, widgets }
    }
}

/// `DeathScreen.extractDeathBackground`.
pub fn death_background(env: &Env, ctx: &mut Ctx) {
    ctx.gui.gradient(0, 0, env.width, env.height, 0x6050_0000, 0xA080_3030);
}

impl Screen for DeathScreen {
    fn background(&mut self, ctx: &mut Ctx) {
        death_background(&self.env, ctx);
    }

    fn foreground(&mut self, ctx: &mut Ctx) {
        render_all(&mut self.widgets, ctx);
        let middle = self.env.width / 2;
        ctx.push(vec![PoseOp::Scale(2.0, 2.0)]);
        ctx.label_centered(self.title.clone(), middle / 2, 30, 1.0);
        ctx.pop();
        if let Some(cause) = &self.cause {
            ctx.label_centered(cause.clone(), middle, 85, 1.0);
        }
        ctx.label_centered(self.score.clone(), middle, 100, 1.0);
    }
}
