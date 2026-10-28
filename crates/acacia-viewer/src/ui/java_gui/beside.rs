//! The screens the options link to that are not option lists (`input::link`'s other pages), on
//! what the viewer has: its key bindings (Java's defaults), its one language and pack.

use acacia_ui::java::screen::credits::credits_and_attribution;
use acacia_ui::java::screen::keys::{self, Os};
use acacia_ui::java::screen::language::{self, Language, Languages};
use acacia_ui::java::screen::packs::{self, Compat, Pack, Packs};
use acacia_ui::java::screen::telemetry::{Telemetry, TelemetryInfoScreen};
use acacia_ui::java::screen::{Env, Screen, keybinds};
use acacia_ui::java::text::Style;
use acacia_ui::java::widget::Ctx;

/// `page`'s screen, or `None` for an option list.
pub fn screen(ctx: &Ctx, env: Env, page: &str) -> Option<Box<dyn Screen>> {
    Some(match page {
        "credits" => Box::new(credits_and_attribution(ctx, env)),
        "keybinds" => {
            // The viewer's keys are fixed, and are Java's defaults; no key waits to be rebound.
            let mappings = keys::defaults();
            let os = if cfg!(target_os = "macos") { Os::Mac } else { Os::Windows };
            Box::new(keybinds::build(ctx, env, &keybinds::KeyBinds { mappings: &mappings, selected: None, os, drawn_before: true }))
        }
        "language" => {
            let languages = [english()];
            let l = Languages { languages: &languages, selected: "en_us", search: "", search_focused: true, focused_time: 0, list_focused: false, scroll: None };
            Box::new(language::build(ctx, env, &l))
        }
        "packs" => {
            let selected = [vanilla(ctx)];
            let p = Packs { title: ctx.tr("resourcePack.title"), available: &[], selected: &selected, search: "", search_focused: true, focused_time: 0, scroll: [0.0; 2], focus: None };
            Box::new(packs::build(ctx, env, &p))
        }
        // No opt-in to extra telemetry: the client sends none.
        "telemetry" => Box::new(TelemetryInfoScreen::new(ctx, env, Telemetry::default(), 0.0)),
        _ => return None,
    })
}

/// The only language the Java look bakes.
fn english() -> Language {
    Language { code: "en_us".into(), region: "United States".into(), name: "English".into() }
}

/// The built-in pack the Java look is drawn from.
fn vanilla(ctx: &Ctx) -> Pack {
    Pack {
        id: "vanilla".into(),
        title: ctx.tr("resourcePack.vanilla.name"),
        description: ctx.tr("resourcePack.vanilla.description"),
        description_style: Style::default(),
        icon: "misc/unknown_pack".into(),
        compat: Compat::Compatible,
        selected: true,
        required: true,
        fixed: true,
        can_move_up: false,
        can_move_down: false,
    }
}
