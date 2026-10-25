//! `Options`' option instances as 26.3 defines them: caption key, value set, default and label.

use super::value::{Def, Label, Set, V};
use crate::java::text::Text;
use crate::java::widget::Ctx;

fn lit(n: impl ToString) -> Text {
    Text::literal(n.to_string())
}

/// `Options.genericValueLabel`.
pub fn generic(ctx: &Ctx, caption: Text, value: Text) -> Text {
    ctx.tr_args("options.generic_value", &[caption, value])
}

fn generic_key(ctx: &Ctx, caption: Text, key: &str) -> Text {
    generic(ctx, caption, ctx.tr(key))
}

fn percent(ctx: &Ctx, caption: Text, v: f64) -> Text {
    ctx.tr_args("options.percent_value", &[caption, lit((v * 100.0) as i32)])
}

fn percent_label(ctx: &Ctx, c: Text, v: &V) -> Text {
    percent(ctx, c, v.f())
}

fn percent_or_off(ctx: &Ctx, c: Text, v: &V) -> Text {
    if v.f() == 0.0 { generic_key(ctx, c, "options.off") } else { percent(ctx, c, v.f()) }
}

fn off_composed(ctx: &Ctx, c: Text) -> Text {
    ctx.tr_args("options.off.composed", &[c])
}

fn pixels(ctx: &Ctx, c: Text, n: i32) -> Text {
    ctx.tr_args("options.pixel_value", &[c, lit(n)])
}

fn chunks(ctx: &Ctx, c: Text, v: &V) -> Text {
    generic(ctx, c, ctx.tr_args("options.chunks", &[lit(v.i())]))
}

/// Java's `Double.toString` for the values these options take (shortest form, `.0` on integers).
fn java_double(v: f64) -> String {
    if v.fract() == 0.0 { format!("{v:.1}") } else { format!("{v}") }
}

fn cycle(caption: &'static str, default: V, label: Label) -> Def {
    Def { caption, set: Set::Cycle, default, label }
}

fn on_off(caption: &'static str, default: bool) -> Def {
    cycle(caption, V::Bool(default), Label::OnOff)
}

fn keys(caption: &'static str, default: &str, keys: &'static [(&'static str, &'static str)]) -> Def {
    cycle(caption, V::Name(default.into()), Label::Keys(keys))
}

fn slider(caption: &'static str, set: Set, default: V, label: fn(&Ctx, Text, &V) -> Text) -> Def {
    Def { caption, set, default, label: Label::F(label) }
}

fn int(caption: &'static str, min: i32, max: i32, default: i32, label: fn(&Ctx, Text, &V) -> Text) -> Def {
    slider(caption, Set::Int { min, max }, V::Int(default), label)
}

fn unit(caption: &'static str, default: f64, label: fn(&Ctx, Text, &V) -> Text) -> Def {
    slider(caption, Set::Unit, V::Num(default), label)
}

fn tenths(min: i32, max: i32) -> Set {
    Set::Mapped { min, max, to: |v| f64::from(v) / 10.0, from: |v| (v * 10.0) as i32 }
}

fn squared() -> Set {
    Set::UnitMapped { to: |v| v * v, from: f64::sqrt }
}

fn toggle_hold(ctx: &Ctx, _: Text, v: &V) -> Text {
    ctx.tr(if v.b() { "options.key.toggle" } else { "options.key.hold" })
}

const CLOUDS: &[(&str, &str)] = &[("OFF", "options.off"), ("FAST", "options.clouds.fast"), ("FANCY", "options.clouds.fancy")];
const PARTICLES: &[(&str, &str)] = &[("ALL", "options.particles.all"), ("DECREASED", "options.particles.decreased"), ("MINIMAL", "options.particles.minimal")];
const CHUNK_UPDATES: &[(&str, &str)] = &[
    ("NONE", "options.prioritizeChunkUpdates.none"),
    ("PLAYER_AFFECTED", "options.prioritizeChunkUpdates.byPlayer"),
    ("NEARBY", "options.prioritizeChunkUpdates.nearby"),
];
const CHAT: &[(&str, &str)] = &[("FULL", "options.chat.visibility.full"), ("SYSTEM", "options.chat.visibility.system"), ("HIDDEN", "options.chat.visibility.hidden")];
const ATTACK: &[(&str, &str)] = &[("OFF", "options.off"), ("CROSSHAIR", "options.attack.crosshair"), ("HOTBAR", "options.attack.hotbar")];
const ARM: &[(&str, &str)] = &[("LEFT", "options.mainHand.left"), ("RIGHT", "options.mainHand.right")];
const INACTIVITY: &[(&str, &str)] = &[("MINIMIZED", "options.inactivityFpsLimit.minimized"), ("AFK", "options.inactivityFpsLimit.afk")];
const FILTERING: &[(&str, &str)] = &[
    ("NONE", "options.textureFiltering.none"),
    ("RGSS", "options.textureFiltering.rgss"),
    ("ANISOTROPIC", "options.textureFiltering.anisotropic"),
];
const GRAPHICS_API: &[(&str, &str)] = &[("DEFAULT", "options.graphicsApi.default"), ("OPENGL", "options.graphicsApi.opengl"), ("VULKAN", "options.graphicsApi.vulkan")];
const MUSIC: &[(&str, &str)] = &[
    ("DEFAULT", "options.music_frequency.default"),
    ("FREQUENT", "options.music_frequency.frequent"),
    ("CONSTANT", "options.music_frequency.constant"),
];
const MUSIC_TOAST: &[(&str, &str)] = &[
    ("NEVER", "options.musicToast.never"),
    ("PAUSE", "options.musicToast.pauseMenu"),
    ("PAUSE_AND_TOAST", "options.musicToast.pauseMenuAndToast"),
];
const PRESENCE: &[(&str, &str)] = &[("NONE", "options.sharePresence.none"), ("LIMITED", "options.sharePresence.limited"), ("ALL", "options.sharePresence.all")];
pub const PRESETS: &[&str] = &["FAST", "FANCY", "FABULOUS", "CUSTOM"];

/// The option `id` (an `Options` getter's name, or `sound.<source name>`).
pub fn def(id: &str) -> Def {
    if let Some(source) = id.strip_prefix("sound.") {
        let caption: &'static str = SOUND_CAPTIONS.iter().find(|c| c.strip_prefix("soundCategory.") == Some(source)).copied().unwrap_or("soundCategory.master");
        return unit(caption, 1.0, percent_or_off);
    }
    match id {
        "darkMojangStudiosBackground" => on_off("options.darkMojangStudiosBackgroundColor", false),
        "hideLightningFlash" => on_off("options.hideLightningFlashes", false),
        "hideSplashTexts" => on_off("options.hideSplashTexts", false),
        "sensitivity" => unit("options.sensitivity", 0.5, |ctx, c, v| {
            if v.f() == 0.0 {
                generic_key(ctx, c, "options.sensitivity.min")
            } else if v.f() == 1.0 {
                generic_key(ctx, c, "options.sensitivity.max")
            } else {
                percent(ctx, c, 2.0 * v.f())
            }
        }),
        "renderDistance" => int("options.renderDistance", 2, 32, 12, chunks),
        "simulationDistance" => int("options.simulationDistance", 5, 32, 12, chunks),
        "entityDistanceScaling" => slider(
            "options.entityDistanceScaling",
            Set::Mapped { min: 2, max: 20, to: |v| f64::from(v) / 4.0, from: |v| (v * 4.0) as i32 },
            V::Num(1.0),
            percent_label,
        ),
        "framerateLimit" => slider(
            "options.framerateLimit",
            Set::Mapped { min: 1, max: 26, to: |v| f64::from(v * 10), from: |v| v as i32 / 10 },
            V::Int(120),
            |ctx, c, v| match v.i() {
                260 => generic_key(ctx, c, "options.framerateLimit.max"),
                n => generic(ctx, c, ctx.tr_args("options.framerate", &[lit(n)])),
            },
        ),
        "preferredGraphicsBackend" => keys("options.graphicsApi", "DEFAULT", GRAPHICS_API),
        "graphicsPreset" => slider("options.graphics.preset", Set::Enum(PRESETS), V::Name("FANCY".into()), |ctx, c, v| {
            generic_key(ctx, c, &format!("options.graphics.{}", v.s().to_lowercase()))
        }),
        "inactivityFpsLimit" => keys("options.inactivityFpsLimit", "AFK", INACTIVITY),
        "cloudStatus" => keys("options.renderClouds", "FANCY", CLOUDS),
        "cloudRange" => int("options.renderCloudsDistance", 2, 128, 128, chunks),
        "weatherRadius" => int("options.weatherRadius", 3, 10, 10, |ctx, c, v| generic(ctx, c, ctx.tr_args("options.blocks", &[lit(v.i())]))),
        "cutoutLeaves" => on_off("options.cutoutLeaves", true),
        "vignette" => on_off("options.vignette", true),
        "improvedTransparency" => on_off("options.improvedTransparency", false),
        "ambientOcclusion" => on_off("options.ao", true),
        "chunkSectionFadeInTime" => slider(
            "options.chunkFade",
            Set::Mapped { min: 0, max: 40, to: |v| f64::from(v) / 20.0, from: |v| (v * 20.0) as i32 },
            V::Num(0.75),
            |ctx, _, v| match v.f() {
                f if f <= 0.0 => ctx.tr("options.chunkFade.none"),
                f => ctx.tr_args("options.chunkFade.seconds", &[lit(format!("{f:.2}"))]),
            },
        ),
        "prioritizeChunkUpdates" => keys("options.prioritizeChunkUpdates", "NONE", CHUNK_UPDATES),
        "chatVisibility" => keys("options.chat.visibility", "FULL", CHAT),
        "chatOpacity" => unit("options.chat.opacity", 1.0, |ctx, c, v| percent(ctx, c, v.f() * 0.9 + 0.1)),
        "chatLineSpacing" => unit("options.chat.line_spacing", 0.0, percent_label),
        "menuBackgroundBlurriness" => int("options.accessibility.menu_background_blurriness", 0, 10, 5, |ctx, c, v| match v.i() {
            0 => generic_key(ctx, c, "options.off"),
            n => generic(ctx, c, lit(n)),
        }),
        "textBackgroundOpacity" => unit("options.accessibility.text_background_opacity", 0.5, percent_label),
        "panoramaSpeed" => unit("options.accessibility.panorama_speed", 1.0, percent_label),
        "highContrast" => on_off("options.accessibility.high_contrast", false),
        "highContrastBlockOutline" => on_off("options.accessibility.high_contrast_block_outline", false),
        "narratorHotkey" => on_off("options.accessibility.narrator_hotkey", true),
        "mainHand" => keys("options.mainHand", "RIGHT", ARM),
        "chatScale" => unit("options.chat.scale", 1.0, |ctx, c, v| if v.f() == 0.0 { off_composed(ctx, c) } else { percent(ctx, c, v.f()) }),
        "chatWidth" => unit("options.chat.width", 1.0, |ctx, c, v| pixels(ctx, c, (v.f() * 280.0 + 40.0).floor() as i32)),
        "chatHeightUnfocused" => unit("options.chat.height.unfocused", 0.4375, |ctx, c, v| pixels(ctx, c, (v.f() * 160.0 + 20.0).floor() as i32)),
        "chatHeightFocused" => unit("options.chat.height.focused", 1.0, |ctx, c, v| pixels(ctx, c, (v.f() * 160.0 + 20.0).floor() as i32)),
        "chatDelay" => slider("options.chat.delay_instant", tenths(0, 60), V::Num(0.0), |ctx, _, v| match v.f() {
            f if f <= 0.0 => ctx.tr("options.chat.delay_none"),
            f => ctx.tr_args("options.chat.delay", &[lit(format!("{f:.1}"))]),
        }),
        "notificationDisplayTime" => slider("options.notifications.display_time", tenths(5, 100), V::Num(1.0), |ctx, c, v| {
            generic(ctx, c, ctx.tr_args("options.multiplier", &[lit(java_double(v.f()))]))
        }),
        "mipmapLevels" => int("options.mipmapLevels", 0, 4, 4, |ctx, c, v| match v.i() {
            0 => off_composed(ctx, c),
            n => generic(ctx, c, lit(n)),
        }),
        "maxAnisotropyBit" => int("options.maxAnisotropy", 1, 3, 2, |ctx, c, v| match v.i() {
            0 => off_composed(ctx, c),
            n => generic(ctx, c, ctx.tr_args("options.multiplier", &[lit(1 << n)])),
        }),
        "textureFiltering" => keys("options.textureFiltering", "NONE", FILTERING),
        "attackIndicator" => keys("options.attackIndicator", "CROSSHAIR", ATTACK),
        "biomeBlendRadius" => int("options.biomeBlendRadius", 0, 7, 2, |ctx, c, v| {
            generic_key(ctx, c, &format!("options.biomeBlendRadius.{}", v.i() * 2 + 1))
        }),
        "mouseWheelSensitivity" => slider(
            "options.mouseWheelSensitivity",
            Set::Mapped { min: -200, max: 100, to: |v| 10f64.powf(f64::from(v) / 100.0), from: |v| (v.log10() * 100.0).floor() as i32 },
            V::Num(1.0),
            |ctx, c, v| generic(ctx, c, lit(format!("{:.2}", v.f()))),
        ),
        "allowCursorChanges" => on_off("options.allowCursorChanges", true),
        "quitShortcuts" => on_off("options.quitShortcuts", true),
        "ctrlClickEmulatesRightClick" => on_off("options.ctrlClickEmulatesRightClick", false),
        "autoJump" => on_off("options.autoJump", false),
        "rotateWithMinecart" => on_off("options.rotateWithMinecart", false),
        "operatorItemsTab" => on_off("options.operatorItemsTab", false),
        "autoSuggestions" => on_off("options.autoSuggestCommands", true),
        "chatColors" => on_off("options.chat.color", true),
        "chatLinks" => on_off("options.chat.links", true),
        "chatLinksPrompt" => on_off("options.chat.links.prompt", true),
        "enableVsync" => on_off("options.vsync", true),
        "entityShadows" => on_off("options.entityShadows", true),
        "forceUnicodeFont" => on_off("options.forceUnicodeFont", false),
        "japaneseGlyphVariants" => on_off("options.japaneseGlyphVariants", false),
        "invertMouseX" => on_off("options.invertMouseX", false),
        "invertMouseY" => on_off("options.invertMouseY", false),
        "discreteMouseScroll" => on_off("options.discrete_mouse_scroll", false),
        "realmsNotifications" => on_off("options.realmsNotifications.button", true),
        "allowServerListing" => on_off("options.allowServerListing", true),
        "reducedDebugInfo" => on_off("options.reducedDebugInfo", false),
        "inGameNotification" => on_off("options.inGameNotification", false),
        "sharePresence" => keys("options.sharePresence", "ALL", PRESENCE),
        "showSubtitles" => on_off("options.showSubtitles", false),
        "directionalAudio" => on_off("options.directionalAudio", false),
        "backgroundForChatOnly" => cycle("options.accessibility.text_background", V::Bool(true), Label::F(|ctx, _, v| {
            ctx.tr(if v.b() { "options.accessibility.text_background.chat" } else { "options.accessibility.text_background.everywhere" })
        })),
        "fullscreen" => on_off("options.fullscreen", false),
        "exclusiveFullscreen" => on_off("options.exclusiveFullscreen", false),
        "bobView" => on_off("options.viewBobbing", true),
        "toggleCrouch" => cycle("key.sneak", V::Bool(false), Label::F(toggle_hold)),
        "toggleSprint" => cycle("key.sprint", V::Bool(false), Label::F(toggle_hold)),
        "toggleAttack" => cycle("key.attack", V::Bool(false), Label::F(toggle_hold)),
        "toggleUse" => cycle("key.use", V::Bool(false), Label::F(toggle_hold)),
        "sprintWindow" => int("options.sprintWindow", 0, 10, 7, |ctx, c, v| match v.i() {
            0 => generic_key(ctx, c, "options.off"),
            n => generic(ctx, c, ctx.tr_args("options.value", &[lit(n)])),
        }),
        "hideMatchedNames" => on_off("options.hideMatchedNames", true),
        "showAutosaveIndicator" => on_off("options.autosaveIndicator", true),
        "onlyShowSecureChat" => on_off("options.onlyShowSecureChat", false),
        "saveChatDrafts" => on_off("options.chat.drafts", false),
        "fov" => int("options.fov", 30, 110, 70, |ctx, c, v| match v.i() {
            70 => generic_key(ctx, c, "options.fov.min"),
            110 => generic_key(ctx, c, "options.fov.max"),
            n => generic(ctx, c, lit(n)),
        }),
        "screenEffectScale" => unit("options.screenEffectScale", 1.0, percent_or_off),
        "fovEffectScale" => slider("options.fovEffectScale", squared(), V::Num(1.0), percent_or_off),
        "darknessEffectScale" => slider("options.darknessEffectScale", squared(), V::Num(1.0), percent_or_off),
        "glintSpeed" => unit("options.glintSpeed", 0.5, percent_or_off),
        "glintStrength" => unit("options.glintStrength", 0.75, percent_or_off),
        "damageTiltStrength" => unit("options.damageTiltStrength", 1.0, percent_or_off),
        "gamma" => unit("options.gamma", 0.5, |ctx, c, v| match (v.f() * 100.0) as i32 {
            0 => generic_key(ctx, c, "options.gamma.min"),
            50 => generic_key(ctx, c, "options.gamma.default"),
            100 => generic_key(ctx, c, "options.gamma.max"),
            n => generic(ctx, c, lit(n)),
        }),
        "guiScale" => cycle("options.guiScale", V::Int(0), Label::F(|ctx, _, v| match v.i() {
            0 => ctx.tr("options.guiScale.auto"),
            n => lit(n),
        })),
        // No narrator library here: `GameNarrator.isActive` is false.
        "narrator" => cycle("options.narrator", V::Name("OFF".into()), Label::F(|ctx, _, _| ctx.tr("options.narrator.notavailable"))),
        "particles" => keys("options.particles", "ALL", PARTICLES),
        "soundDevice" => cycle("options.audioDevice", V::Name(String::new()), Label::F(|ctx, _, v| match v.s() {
            "" => ctx.tr("options.audioDevice.default"),
            s => Text::literal(s.strip_prefix("OpenAL Soft on ").unwrap_or(s)),
        })),
        "musicFrequency" => keys("options.music_frequency", "DEFAULT", MUSIC),
        "musicToast" => keys("options.musicToast", "NEVER", MUSIC_TOAST),
        _ => on_off("", false),
    }
}

/// `SoundSource.values()`' captions, in order.
pub const SOUND_CAPTIONS: &[&str] = &[
    "soundCategory.master",
    "soundCategory.music",
    "soundCategory.record",
    "soundCategory.weather",
    "soundCategory.block",
    "soundCategory.hostile",
    "soundCategory.neutral",
    "soundCategory.player",
    "soundCategory.ambient",
    "soundCategory.voice",
    "soundCategory.ui",
];
