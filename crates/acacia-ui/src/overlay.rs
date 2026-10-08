//! What servers show over the HUD: the title and subtitle, the action bar and boss bars, laid out
//! as Java's `Gui` and `BossHealthOverlay` lay them out.

use std::time::{Duration, Instant};

use crate::draw::{DrawList, WHITE};
use crate::font::Font;
use crate::theme::Theme;

/// Java's default title timing in ticks: fade in, stay, fade out.
const TITLE_TIMES: [u32; 3] = [10, 70, 20];
/// The action bar stays 3 s and fades over its last half.
const ACTION_BAR: Duration = Duration::from_secs(3);
const TICK: f32 = 0.05;
pub const BOSS_COLOURS: [&str; 7] = ["pink", "blue", "red", "green", "yellow", "purple", "white"];

#[derive(Default)]
pub struct Titles {
    title: Option<(String, Instant)>,
    subtitle: Option<String>,
    action_bar: Option<(String, Instant)>,
}

impl Titles {
    /// A title shows its subtitle, set before or after it, until it fades.
    pub fn title(&mut self, text: String, now: Instant) {
        self.title = Some((text, now));
    }

    pub fn subtitle(&mut self, text: String) {
        self.subtitle = Some(text);
    }

    pub fn action_bar(&mut self, text: String, now: Instant) {
        self.action_bar = Some((text, now));
    }

    pub fn clear(&mut self) {
        (self.title, self.subtitle) = (None, None);
    }
}

/// 0 to 1 for a title shown since `since`; `None` once it has gone.
fn title_alpha(since: Instant, now: Instant) -> Option<f32> {
    let t = now.saturating_duration_since(since).as_secs_f32() / TICK;
    let [fade_in, stay, fade_out] = TITLE_TIMES.map(|v| v as f32);
    match t {
        t if t < fade_in => Some(t / fade_in),
        t if t < fade_in + stay => Some(1.0),
        t if t < fade_in + stay + fade_out => Some(1.0 - (t - fade_in - stay) / fade_out),
        _ => None,
    }
}

/// `text` centred on `x`, `scale` times the font's size, its top at `y` (GUI pixels).
fn centred(list: &mut DrawList, font: &Font, text: &str, x: f32, y: f32, scale: f32, alpha: f32) {
    let base = list.scale;
    list.scale = base * scale;
    font.draw(list, text, x / scale - font.width(text) / 2.0, y / scale, 0xFFFFFF, alpha, true);
    list.scale = base;
}

pub fn draw_titles(list: &mut DrawList, theme: &Theme, titles: &Titles, now: Instant, [w, h]: [f32; 2]) {
    let Some(font) = &theme.font else { return };
    if let Some((text, since)) = &titles.title
        && let Some(alpha) = title_alpha(*since, now)
    {
        // Java: scale 4 at the screen's centre less 10 (scaled) pixels, the subtitle at scale 2, 5 below.
        centred(list, font, text, w / 2.0, h / 2.0 - 40.0, 4.0, alpha);
        if let Some(sub) = &titles.subtitle {
            centred(list, font, sub, w / 2.0, h / 2.0 + 10.0, 2.0, alpha);
        }
    }
    if let Some((text, since)) = &titles.action_bar {
        let left = ACTION_BAR.saturating_sub(now.saturating_duration_since(*since)).as_secs_f32();
        if left > 0.0 {
            centred(list, font, text, w / 2.0, h - 68.0, 1.0, (left / (ACTION_BAR.as_secs_f32() / 2.0)).min(1.0));
        }
    }
}

/// One boss bar: its name, 0 to 1 filled, and an entry of [`BOSS_COLOURS`].
pub struct Boss<'a> {
    pub title: &'a str,
    pub progress: f32,
    pub colour: usize,
}

/// Bars from the top centre down, 19 pixels apart, each name 9 above its bar.
pub fn draw_bosses(list: &mut DrawList, theme: &Theme, bosses: &[Boss], [w, _]: [f32; 2]) {
    let left = (w / 2.0).floor() - 91.0;
    for (i, boss) in bosses.iter().enumerate() {
        let y = 12.0 + i as f32 * 19.0;
        let colour = BOSS_COLOURS.get(boss.colour).unwrap_or(&"pink");
        if let Some(bg) = theme.atlas.get(&format!("boss_bar/{colour}_background")) {
            list.sprite(bg, left, y, WHITE);
        }
        if let Some(fill) = theme.atlas.get(&format!("boss_bar/{colour}_progress")) {
            let filled = (boss.progress.clamp(0.0, 1.0) * 183.0).floor().min(fill.width as f32);
            list.sprite_part(fill, left, y, [0.0, 0.0, filled, fill.height as f32], WHITE);
        }
        if let Some(font) = &theme.font {
            font.draw(list, boss.title, (w / 2.0 - font.width(boss.title) / 2.0).floor(), y - 9.0, 0xFFFFFF, 1.0, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_fade_in_stay_and_out() {
        let start = Instant::now();
        let at = |ticks: u32| title_alpha(start, start + Duration::from_millis(u64::from(ticks) * 50));
        assert_eq!(at(5), Some(0.5));
        assert_eq!(at(40), Some(1.0));
        assert_eq!(at(90), Some(0.5));
        assert_eq!(at(101), None);
    }
}
