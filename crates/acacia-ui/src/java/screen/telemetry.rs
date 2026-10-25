//! `TelemetryInfoScreen`: the title, description and link buttons over a scrolling
//! `TelemetryEventWidget` listing every telemetry event and its properties, the opt-in checkbox
//! (when extra telemetry is offered) and the footer buttons.

use super::{Env, Screen, arena_children, menu_background, render_all};
use crate::java::gui::{Metrics, PoseOp};
use crate::java::text::{Style, Text};
use crate::java::widget::button::{Button, Checkbox};
use crate::java::widget::header_footer::HeaderFooter;
use crate::java::widget::input::{Child, Interaction};
use crate::java::widget::layout::{Arena, Elem, Linear};
use crate::java::widget::text::{MultiLineText, MultiLineTextWidget, StringWidget};
use crate::java::widget::text_area::{INNER_PADDING, TextArea};
use crate::java::widget::{Base, Ctx, Widget, render};

const GLOBAL: &[&str] = &[
    "user_id", "client_id", "minecraft_session_id", "game_version", "operating_system", "platform", "client_modded", "launcher_name",
    "event_timestamp_utc", "opt_in",
];
const WORLD: &[&str] = &["world_session_id", "server_modded", "server_type", "server_session_id"];

/// `TelemetryEventType`s in registration order: id, opt-in, whether the world session's
/// properties follow the global ones, and the event's own properties.
const EVENTS: &[(&str, bool, bool, &[&str])] = &[
    ("graphics_capabilities", false, false, &["backend_name", "backend_failure_message", "backend_failure_reason", "backend_failure_missing_capabilities"]),
    ("world_loaded", false, true, &["game_mode", "realms_map_content"]),
    ("performance_metrics", true, true, &["frame_rate_samples", "render_time_samples", "used_memory_samples", "number_of_samples", "render_distance", "dedicated_memory_kb"]),
    ("world_load_times", true, true, &["world_load_time_ms", "new_world"]),
    ("world_unloaded", false, true, &["seconds_since_load", "ticks_since_load"]),
    ("advancement_made", true, true, &["advancement_id", "advancement_game_time"]),
    ("game_load_times", true, false, &["load_time_total_time_ms", "load_time_pre_window_ms", "load_time_bootstrap_ms", "load_time_loading_overlay_ms"]),
];

const GRAY: u32 = 0xAA_AAAA;

/// What the screen reads from the client.
#[derive(Clone, Copy, Debug, Default)]
pub struct Telemetry {
    /// `extraTelemetryAvailable` (the opt-in checkbox and a taller footer).
    pub extra_available: bool,
    /// `options.telemetryOptInExtra`.
    pub opt_in: bool,
}

impl Telemetry {
    /// `Minecraft.telemetryOptInExtra`.
    fn opted_in(&self) -> bool {
        self.extra_available && self.opt_in
    }
}

/// `TelemetryEventWidget`: the events laid out in a column, scrolled in a text area.
pub struct EventWidget {
    pub area: TextArea,
    widgets: Arena,
    inner_height: i32,
}

fn gray_if(t: Text, disabled: bool) -> Text {
    if disabled { t.with_color(GRAY) } else { t }
}

impl EventWidget {
    /// `new TelemetryEventWidget(0, 0, w, h, font)` and its `buildContent(optedIn)`.
    pub fn new(ctx: &Ctx, w: i32, h: i32, opted_in: bool) -> EventWidget {
        let width = w - INNER_PADDING * 2;
        let mut widgets: Arena = Vec::new();
        let mut column = Linear::new(true, 0);
        column.add_default(Elem::Spacer { x: 0, y: 0, w: width, h: 0 });
        let mut events: Vec<_> = EVENTS.to_vec();
        events.sort_by_key(|e| e.1);
        let mut add = |column: &mut Linear, text: Text, header: bool, pad_bottom: i32| {
            let mut t = MultiLineText::new(text);
            t.max_width = Some(if header { width - 64 } else { width });
            t.centered = header;
            widgets.push(Box::new(MultiLineTextWidget::new(ctx.font, t)));
            let d = column.grid.defaults;
            let s = if header { d.align_x(0.5).padding(32, 0, 32, 0) } else { d.pad_bottom(pad_bottom) };
            column.add(Elem::Widget(widgets.len() - 1), s);
        };
        for (i, &(id, opt_in, world, own)) in events.iter().enumerate() {
            let disabled = opt_in && !opted_in;
            let pattern = match (opt_in, disabled) {
                (false, _) => "telemetry.event.required",
                (true, false) => "telemetry.event.optional",
                (true, true) => "telemetry.event.optional.disabled",
            };
            let title = ctx.tr(&format!("telemetry.event.{id}.title"));
            add(&mut column, gray_if(ctx.tr_args(pattern, &[title]), disabled), true, 0);
            add(&mut column, ctx.tr(&format!("telemetry.event.{id}.description")).with_color(GRAY), true, 0);
            column.add_default(Elem::Spacer { x: 0, y: 0, w: 0, h: 9 / 2 });
            let property_title = ctx.tr("telemetry_info.property_title").with_style(&Style { underlined: Some(true), ..Style::default() });
            add(&mut column, gray_if(property_title, disabled), false, 2);
            let world_props = if world { WORLD } else { &[] };
            for p in GLOBAL.iter().chain(world_props).chain(own.iter()) {
                add(&mut column, gray_if(ctx.tr(&format!("telemetry.property.{p}.title")), disabled), false, 0);
            }
            if i + 1 < events.len() {
                column.add_default(Elem::Spacer { x: 0, y: 0, w: 0, h: 9 });
            }
        }
        let mut root = column.into_elem();
        let inner_height = match &mut root {
            Elem::Layout(l) => {
                l.arrange(&mut widgets);
                l.h
            }
            _ => 0,
        };
        let mut area = TextArea::new(0, 0, w, h, 9.0);
        area.base.message = Text::default();
        EventWidget { area, widgets, inner_height }
    }

    pub fn set_scroll(&mut self, v: f64) {
        self.area.set_scroll(v, self.inner_height);
    }
}

impl Widget for EventWidget {
    fn base(&self) -> &Base {
        &self.area.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.area.base
    }
    fn extract(&mut self, ctx: &mut Ctx, _hovered: bool) {
        let (area, widgets) = (&self.area, &mut self.widgets);
        let (left, top) = (area.inner_left(), area.inner_top());
        area.extract(ctx, self.inner_height, |ctx| {
            ctx.push(vec![PoseOp::Translate(left as f32, top as f32)]);
            render_all(widgets, ctx);
            ctx.pop();
        }, |_| {});
    }
}

pub const PRIVACY: &str = "telemetry_info.button.privacy_statement";
pub const FEEDBACK: &str = "telemetry_info.button.give_feedback";
pub const SHOW_DATA: &str = "telemetry_info.button.show_data";
pub const OPT_IN: &str = "telemetry_info.opt_in.description";
pub const DONE: &str = "gui.done";

pub struct TelemetryInfoScreen {
    pub env: Env,
    /// The header's widgets, then the footer's.
    widgets: Arena,
    /// How many of `widgets` are the header's: the event widget's index among the children.
    split: usize,
    pub events: EventWidget,
}

/// Where the layout put the spacer standing in for the event widget.
fn spacer_at(e: &Elem) -> Option<(i32, i32)> {
    match e {
        Elem::Spacer { x, y, .. } => Some((*x, *y)),
        Elem::Layout(l) => l.cells.iter().find_map(|c| spacer_at(&c.elem)),
        Elem::Widget(_) => None,
    }
}

impl TelemetryInfoScreen {
    /// `init()` and `repositionElements` with the event list scrolled to `scroll`.
    pub fn new(ctx: &Ctx, env: Env, t: Telemetry, scroll: f64) -> TelemetryInfoScreen {
        let m: &dyn Metrics = ctx.font;
        let mut layout = HeaderFooter::new(16 + 9 * 5 + 20, if t.extra_available { 33 + 9 + 8 } else { 33 });
        let mut widgets: Arena = Vec::new();
        let push = |widgets: &mut Arena, w: Box<dyn Widget>| {
            widgets.push(w);
            Elem::Widget(widgets.len() - 1)
        };
        let mut header = Linear::new(true, 4);
        header.grid.defaults = header.grid.defaults.align_x(0.5);
        header.add_default(push(&mut widgets, Box::new(StringWidget::fit(m, ctx.tr("telemetry_info.screen.title")))));
        let mut description = MultiLineText::new(ctx.tr("telemetry_info.screen.description").with_color(0xBA_BABA));
        description.centered = true;
        description.max_width = Some(env.width - 16);
        header.add_default(push(&mut widgets, Box::new(MultiLineTextWidget::new(m, description))));
        let mut links = Linear::new(false, 8);
        links.add_default(push(&mut widgets, Box::new(Button::named(ctx, 150, PRIVACY))));
        links.add_default(push(&mut widgets, Box::new(Button::named(ctx, 150, FEEDBACK))));
        header.add_default(links.into_elem());
        layout.add_header(header.into_elem());
        let mut contents = Linear::new(true, 8);
        let mut events = EventWidget::new(ctx, env.width - 40, layout.content_height(env.height), t.opted_in());
        events.set_scroll(scroll);
        let split = widgets.len();
        contents.add_default(Elem::Spacer { x: 0, y: 0, w: events.area.base.w, h: events.area.base.h });
        layout.add_contents(contents.into_elem());
        let mut footer = Linear::new(true, 4);
        footer.grid.defaults = footer.grid.defaults.align_x(0.5);
        if t.extra_available {
            let message = ctx.tr(OPT_IN).with_color(0xE0_E0E0);
            let mut checkbox = Checkbox::new(m, 0, 0, Some(env.width - 40), message, t.opt_in);
            checkbox.base.id = OPT_IN.into();
            footer.add_default(push(&mut widgets, Box::new(checkbox)));
        }
        let mut buttons = Linear::new(false, 8);
        buttons.add_default(push(&mut widgets, Box::new(Button::named(ctx, 150, SHOW_DATA))));
        buttons.add_default(push(&mut widgets, Box::new(Button::named(ctx, 150, DONE))));
        footer.add_default(buttons.into_elem());
        layout.add_footer(footer.into_elem());
        layout.arrange(&mut widgets, env.width, env.height);
        if let Some((x, y)) = layout.contents.cells.iter().find_map(|c| spacer_at(&c.elem)) {
            (events.area.base.x, events.area.base.y) = (x, y);
        }
        TelemetryInfoScreen { env, widgets, split, events }
    }

    /// The child `setInitialFocus` focuses: the event list.
    pub fn initial_focus(&self) -> usize {
        self.split
    }
}

impl Screen for TelemetryInfoScreen {
    fn background(&mut self, ctx: &mut Ctx) {
        menu_background(&self.env, ctx);
    }

    fn foreground(&mut self, ctx: &mut Ctx) {
        let (header, footer) = self.widgets.split_at_mut(self.split);
        render_all_slice(header, ctx);
        render(&mut self.events, ctx);
        render_all_slice(footer, ctx);
    }

    fn children(&mut self) -> Vec<Child<'_>> {
        let (header, footer) = self.widgets.split_at_mut(self.split);
        let mut out = arena_children(header);
        out.push(Child { widget: &mut self.events, clip: None });
        out.extend(arena_children(footer));
        out
    }

    fn mouse_scrolled(&mut self, s: &mut Interaction, x: f64, y: f64, amount: f64) -> bool {
        let e = &mut self.events;
        if !e.area.over(x, y) {
            return false;
        }
        e.set_scroll(e.area.scroll - amount * e.area.scroll_rate);
        s.scroll = e.area.scroll;
        true
    }
}

fn render_all_slice(widgets: &mut [Box<dyn Widget>], ctx: &mut Ctx) {
    for w in widgets {
        render(w.as_mut(), ctx);
    }
}
