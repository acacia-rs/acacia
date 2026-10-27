//! `CreditsAndAttributionScreen`: the Credits, Attribution and Licenses buttons under a title.

use super::Env;
use super::options::{Build, OptionsScreen};
use crate::java::widget::Ctx;
use crate::java::widget::button::Button;
use crate::java::widget::layout::Linear;
use crate::java::widget::text::StringWidget;

/// The buttons' ids: the credits roll (`WinScreen`), the two links and Done.
pub const CREDITS: &str = "credits_and_attribution.button.credits";
pub const ATTRIBUTION: &str = "credits_and_attribution.button.attribution";
pub const LICENSES: &str = "credits_and_attribution.button.licenses";

/// `init` at the env's size.
pub fn credits_and_attribution(ctx: &Ctx, env: Env) -> OptionsScreen {
    let mut b = Build::new(33, 33);
    let title = b.add(StringWidget::fit(ctx.font, ctx.tr("credits_and_attribution.screen.title")));
    b.layout.add_header(title);
    let mut content = Linear::new(true, 8);
    content.grid.defaults = content.grid.defaults.align_x(0.5);
    for key in [CREDITS, ATTRIBUTION, LICENSES] {
        content.add_default(b.add(Button::named(ctx, 210, key)));
    }
    b.layout.add_contents(content.into_elem());
    let done = b.add(Button::named(ctx, 200, super::options::pages::DONE));
    b.layout.add_footer(done);
    b.finish(env)
}
