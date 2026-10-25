//! Each options screen's `init`: `OptionsScreen` and the `OptionsSubScreen` pages' `addOptions`.

use super::value::{Values, create_button, value};
use super::{Build, OptionsScreen};
use crate::java::screen::Env;
use crate::java::text::{Style, Text};
use crate::java::widget::button::{Button, CycleButton, Display};
use crate::java::widget::layout::{Linear, Layout, RowHelper, Settings};
use crate::java::widget::text::StringWidget;
use crate::java::widget::{Ctx, Widget};

/// What the pages read from the client besides the options.
#[derive(Clone, Copy, Debug, Default)]
pub struct Client {
    /// `allowsTelemetry`.
    pub telemetry: bool,
    /// The `high_contrast` pack is available.
    pub high_contrast_pack: bool,
    /// The level has the minecart-improvements feature.
    pub minecart: bool,
    /// The social manager's friend list and friend requests.
    pub friends: bool,
    pub friend_requests: bool,
    /// Ticked once after init (the video page then greys max anisotropy without anisotropic filtering).
    pub ticked: bool,
}

/// The page named `page` (`options` for `OptionsScreen` itself) after `init`.
pub fn build(ctx: &Ctx, env: Env, page: &str, values: Values, client: Client) -> Option<OptionsScreen> {
    let p = Page { ctx, env, values, client };
    Some(match page {
        "options" => p.main(),
        "video" => p.video(),
        "sound" => p.sub("options.sounds.title", |p, b| {
            p.big(b, "sound.master");
            p.small_ids(b, &["sound.music", "sound.record", "sound.weather", "sound.block", "sound.hostile", "sound.neutral", "sound.player", "sound.ambient", "sound.voice", "sound.ui"]);
            p.big(b, "soundDevice");
            p.small_ids(b, &["showSubtitles", "directionalAudio", "musicFrequency", "musicToast"]);
        }),
        "controls" => p.sub("controls.title", |p, b| {
            let buttons = vec![p.button(150, "options.mouse_settings"), p.button(150, "controls.keybinds")];
            p.list(b).add_small(buttons);
            p.small_ids(b, &["toggleCrouch", "toggleSprint", "toggleAttack", "toggleUse", "autoJump", "sprintWindow", "quitShortcuts", "operatorItemsTab"]);
        }),
        "mouse" => p.sub("options.mouse_settings.title", |p, b| {
            p.small_ids(b, &["sensitivity", "mouseWheelSensitivity", "discreteMouseScroll", "invertMouseX", "invertMouseY", "allowCursorChanges"]);
        }),
        "chat" => p.sub("options.chat.title", |p, b| p.small_ids(b, CHAT)),
        "accessibility" => p.accessibility(),
        "skin" => p.sub("options.skinCustomisation.title", |p, b| {
            let part = |part: &&str| p.on_off(&format!("options.modelPart.{part}"), value(p.values, &format!("modelPart.{part}")).b());
            let mut widgets: Vec<Box<dyn Widget>> = PARTS.iter().map(part).collect();
            widgets.push(create_button(p.ctx, p.values, "mainHand", 150));
            p.list(b).add_small(widgets);
        }),
        "font" => p.sub("options.font.title", |p, b| p.small_ids(b, &["forceUnicodeFont", "japaneseGlyphVariants"])),
        "online" => p.sub("options.online.title", |p, b| p.online(b)),
        _ => return None,
    })
}

const PARTS: &[&str] = &["cape", "jacket", "left_sleeve", "right_sleeve", "left_pants_leg", "right_pants_leg", "hat"];

const CHAT: &[&str] = &[
    "chatVisibility", "chatColors", "chatLinks", "chatLinksPrompt", "chatOpacity", "textBackgroundOpacity", "chatScale", "chatLineSpacing", "chatDelay",
    "chatWidth", "chatHeightFocused", "chatHeightUnfocused", "narrator", "autoSuggestions", "hideMatchedNames", "reducedDebugInfo", "onlyShowSecureChat",
    "saveChatDrafts",
];

/// After the first row of the narrator and the Controls link.
const ACCESSIBILITY: &[&str] = &[
    "showSubtitles", "highContrast", "menuBackgroundBlurriness", "textBackgroundOpacity", "backgroundForChatOnly", "chatOpacity", "chatLineSpacing", "chatDelay",
    "notificationDisplayTime", "bobView", "screenEffectScale", "fovEffectScale", "darknessEffectScale", "damageTiltStrength", "glintSpeed", "glintStrength",
    "hideLightningFlash", "darkMojangStudiosBackground", "panoramaSpeed", "hideSplashTexts", "narratorHotkey", "rotateWithMinecart", "highContrastBlockOutline",
];

const DISPLAY: &[&str] = &["framerateLimit", "enableVsync", "inactivityFpsLimit", "guiScale", "fullscreen", "exclusiveFullscreen", "gamma", "preferredGraphicsBackend"];

const QUALITY: &[&str] = &[
    "biomeBlendRadius", "renderDistance", "prioritizeChunkUpdates", "simulationDistance", "ambientOcclusion", "cloudStatus", "particles", "mipmapLevels",
    "entityShadows", "entityDistanceScaling", "menuBackgroundBlurriness", "cloudRange", "cutoutLeaves", "improvedTransparency", "textureFiltering",
    "maxAnisotropyBit", "weatherRadius",
];

struct Page<'a, 'b> {
    ctx: &'a Ctx<'b>,
    env: Env,
    values: Values<'a>,
    client: Client,
}

/// A list section header: underlined and bold.
fn section(ctx: &Ctx, key: &str) -> Text {
    ctx.tr(key).with_style(&Style { bold: Some(true), underlined: Some(true), ..Style::default() })
}

impl Page<'_, '_> {
    fn button(&self, w: i32, key: &str) -> Box<dyn Widget> {
        Box::new(Button::sized(w, self.ctx.tr(key)))
    }

    /// `CycleButton.onOffBuilder(value).create(name)`.
    fn on_off(&self, name: &str, on: bool) -> Box<dyn Widget> {
        let shown = self.ctx.tr(if on { "options.on" } else { "options.off" });
        Box::new(CycleButton::new(self.ctx.lang, 0, 0, 150, 20, self.ctx.tr(name), shown, Display::NameAndValue))
    }

    fn list<'c>(&self, b: &'c mut Build) -> &'c mut super::list::OptionsList {
        let env = self.env;
        if b.list.is_none() {
            b.add_list(&env);
        }
        b.list.as_mut().expect("added")
    }

    fn big(&self, b: &mut Build, id: &str) {
        let w = create_button(self.ctx, self.values, id, 310);
        self.list(b).add_big(w);
    }

    fn small_ids(&self, b: &mut Build, ids: &[&str]) {
        self.small_with(b, ids, |_, _| {});
    }

    /// `addSmall(options)`, `adjust` seeing each widget after it is made.
    fn small_with(&self, b: &mut Build, ids: &[&str], adjust: impl Fn(&str, &mut Box<dyn Widget>)) {
        let widgets = ids.iter().map(|id| {
            let mut w = self.option(id);
            adjust(id, &mut w);
            w
        });
        self.list(b).add_small(widgets.collect());
    }

    /// `createButton(options)`; `OptionsSubScreen.addContents` greys the narrator's (no narrator here).
    fn option(&self, id: &str) -> Box<dyn Widget> {
        let mut w = create_button(self.ctx, self.values, id, 150);
        if id == "narrator" {
            w.base_mut().active = false;
        }
        w
    }

    /// An `OptionsSubScreen`: the title header, the list `options` fills and a Done footer.
    fn sub(&self, title: &str, options: impl FnOnce(&Self, &mut Build)) -> OptionsScreen {
        let mut b = Build::new(33, 33);
        let title = b.add(StringWidget::fit(self.ctx.font, self.ctx.tr(title)));
        b.layout.add_header(title);
        self.list(&mut b);
        options(self, &mut b);
        let done = b.add(Button::sized(200, self.ctx.tr("gui.done")));
        b.layout.add_footer(done);
        b.finish(self.env)
    }

    fn accessibility(&self) -> OptionsScreen {
        let mut b = Build::new(33, 33);
        let title = b.add(StringWidget::fit(self.ctx.font, self.ctx.tr("options.accessibility.title")));
        b.layout.add_header(title);
        let c = self.client;
        let first = vec![self.option("narrator"), self.button(150, "options.controls")];
        self.list(&mut b).add_small(first);
        self.small_with(&mut b, ACCESSIBILITY, |id, w| match id {
            "highContrast" => w.base_mut().active = c.high_contrast_pack,
            "rotateWithMinecart" => w.base_mut().active = c.minecart,
            _ => {}
        });
        let mut footer = Linear::new(false, 8);
        footer.add_default(b.add(Button::sized(150, self.ctx.tr("options.accessibility.link"))));
        footer.add_default(b.add(Button::sized(150, self.ctx.tr("gui.done"))));
        b.layout.add_footer(footer.into_elem());
        b.finish(self.env)
    }

    fn video(&self) -> OptionsScreen {
        let mut b = Build::new(33, 33);
        let mut header = Linear::new(true, 2);
        header.grid.defaults = header.grid.defaults.align_x(0.5).align_y(0.5);
        header.add_default(b.add(StringWidget::fit(self.ctx.font, self.ctx.tr("options.videoTitle"))));
        if value(self.values, "preferredGraphicsBackend").s() != "DEFAULT" {
            let warning = self.ctx.tr("options.restartRequired").with_color(0xDF_5050);
            header.add_default(b.add(StringWidget::fit(self.ctx.font, warning)));
        }
        b.layout.add_header(header.into_elem());
        self.list(&mut b).add_header(self.ctx, section(self.ctx, "options.video.display.header"));
        // No monitor: the exclusive mode slider ranges over -1 alone.
        let mode = Box::new(crate::java::widget::slider::Slider::new(0, 0, 310, 20, self.ctx.tr("options.fullscreen.unavailable"), 0.0));
        self.list(&mut b).add_big(mode);
        self.small_ids(&mut b, DISPLAY);
        self.list(&mut b).add_header(self.ctx, section(self.ctx, "options.video.quality.header"));
        self.big(&mut b, "graphicsPreset");
        let anisotropic = !self.client.ticked || value(self.values, "textureFiltering").s() == "ANISOTROPIC";
        self.small_with(&mut b, QUALITY, |id, w| {
            if id == "maxAnisotropyBit" {
                w.base_mut().active = anisotropic;
            }
        });
        self.list(&mut b).add_header(self.ctx, section(self.ctx, "options.video.preferences.header"));
        self.small_ids(&mut b, &["showAutosaveIndicator", "vignette", "attackIndicator", "chunkSectionFadeInTime"]);
        let done = b.add(Button::sized(200, self.ctx.tr("gui.done")));
        b.layout.add_footer(done);
        b.finish(self.env)
    }

    fn online(&self, b: &mut Build) {
        let c = self.client;
        self.list(b).add_header(self.ctx, section(self.ctx, "options.online.friends.header"));
        let mut requests = self.on_off("options.allowFriendRequests", c.friend_requests);
        requests.base_mut().active = c.friends;
        self.list(b).add_small(vec![self.on_off("options.friendsList", c.friends), requests]);
        let mut notifications = self.on_off("options.inGameNotification", value(self.values, "inGameNotification").b());
        notifications.base_mut().active = c.friends;
        let mut presence = create_button(self.ctx, self.values, "sharePresence", 150);
        presence.base_mut().active = c.friends;
        self.list(b).add_small(vec![notifications, presence]);
        self.list(b).add_big(self.button(150, "options.online.xboxSettings"));
        self.list(b).add_header(self.ctx, section(self.ctx, "options.online.servers.header"));
        self.big(b, "allowServerListing");
        self.list(b).add_header(self.ctx, section(self.ctx, "options.online.realms.header"));
        self.big(b, "realmsNotifications");
    }

    /// `OptionsScreen.init`.
    fn main(&self) -> OptionsScreen {
        let ctx = self.ctx;
        let mut b = Build::new(61, 33);
        let mut header = Linear::new(true, 8);
        let title = b.add(StringWidget::fit(ctx.font, ctx.tr("options.title")));
        let d = header.grid.defaults;
        header.add(title, d.align_x(0.5));
        let mut sub = Linear::new(false, 8);
        b.widgets.push(create_button(ctx, self.values, "fov", 150));
        sub.add_default(crate::java::widget::layout::Elem::Widget(b.widgets.len() - 1));
        sub.add_default(b.add(Button::sized(150, ctx.tr("options.online"))));
        header.add_default(sub.into_elem());
        b.layout.add_header(header.into_elem());
        let mut grid = Layout::grid();
        grid.defaults = Settings::default().padding(4, 0, 4, 4).align_x(0.5);
        let mut helper = RowHelper::new(2);
        for key in MAIN_BUTTONS {
            let mut button = Button::sized(150, ctx.tr(key));
            button.base.active = *key != "options.telemetry" || self.client.telemetry;
            let e = b.add(button);
            let d = grid.defaults;
            helper.add(&mut grid, e, 1, d);
        }
        b.layout.add_contents(crate::java::widget::layout::Elem::Layout(Box::new(grid)));
        let done = b.add(Button::sized(200, ctx.tr("gui.done")));
        b.layout.add_footer(done);
        b.finish(self.env)
    }
}

const MAIN_BUTTONS: &[&str] = &[
    "options.skinCustomisation", "options.sounds", "options.video", "options.controls", "options.language", "options.chat", "options.resourcepack",
    "options.accessibility", "options.telemetry", "options.credits_and_attribution",
];
