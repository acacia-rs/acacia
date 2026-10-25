//! `SubtitleOverlay`: captions for the sounds the player can hear, with direction arrows, fading
//! over the notification display time. It hears the sound engine only between a frame drawn with
//! "Show Subtitles" on and one drawn with it off. Ported from Horizon's `hud_overlays.rs` (MIT).

use crate::java::gui::{Gui, Metrics, PoseOp};
use crate::java::mth::{argb, floor};
use crate::java::text::Text;

type Vec3 = [f64; 3];

/// `SoundPlayedAt`: where and when (`Util.getMillis`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct PlayedAt {
    location: Vec3,
    time: i64,
}

fn distance_sqr(a: Vec3, b: Vec3) -> f64 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    dx * dx + dy * dy + dz * dz
}

/// `Vec3.closerThan`.
fn closer_than(a: Vec3, b: Vec3, range: f32) -> bool {
    distance_sqr(a, b) < f64::from(range) * f64::from(range)
}

/// `Vec3.equals`: `Double.compare` per axis, so -0.0 differs from 0.0 and NaN equals NaN.
fn same_location(a: Vec3, b: Vec3) -> bool {
    let bits = |v: f64| if v.is_nan() { f64::NAN.to_bits() } else { v.to_bits() };
    (0..3).all(|i| bits(a[i]) == bits(b[i]))
}

#[derive(Clone, Debug)]
struct Subtitle {
    key: String,
    text: Text,
    range: f32,
    played: Vec<PlayedAt>,
    looping: bool,
}

impl Subtitle {
    /// `getBestSubtitleCandidate`: the only play, else the latest in range (the farthest of equally
    /// recent ones, the first of full ties).
    fn best_candidate(&self, position: Vec3) -> Option<PlayedAt> {
        if let [only] = self.played.as_slice() {
            return Some(*only);
        }
        self.played.iter().filter(|p| closer_than(p.location, position, self.range)).fold(None, |best: Option<PlayedAt>, p| match best {
            Some(b) if b.time > p.time || b.time == p.time && distance_sqr(b.location, position).sqrt() >= distance_sqr(p.location, position).sqrt() => Some(b),
            _ => Some(*p),
        })
    }

    fn audible_from(&self, camera: Vec3) -> bool {
        self.range.is_infinite() || self.best_candidate(camera).is_some_and(|p| closer_than(camera, p.location, self.range))
    }

    /// `getBrightness`: 255 down to 75 over the display time; looping sounds hold 255 for 500 ms.
    fn brightness(&self, at: PlayedAt, multiplier: f64, now: i64) -> i32 {
        let full = if self.looping { 500 } else { 0 };
        let t = (now - at.time - full).max(0) as f32 / ((3000 - full) as f64 * multiplier) as f32;
        let v = if t < 0.0 {
            255.0
        } else if t > 1.0 {
            75.0
        } else {
            255.0 + t * (75.0 - 255.0)
        };
        floor(v)
    }
}

/// The listener and options a frame reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct SubtitleFrame {
    /// `options.showSubtitles`.
    pub show: bool,
    /// `SoundManager.getListenerTransform`: position, forward and up.
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    /// `options.notificationDisplayTime`.
    pub multiplier: f64,
    /// `options.getBackgroundColor(0.8F)`.
    pub background: u32,
}

#[derive(Clone, Debug, Default)]
pub struct SubtitleOverlay {
    subtitles: Vec<Subtitle>,
    listening: bool,
}

impl SubtitleOverlay {
    /// Whether the sound engine reports plays to the overlay.
    pub fn listening(&self) -> bool {
        self.listening
    }

    /// `onPlaySound` at `now`: `subtitle` is the event's caption, if it has one, as its translation
    /// key (what the components' `equals` compares) and text; one already listed is refreshed.
    pub fn on_play(&mut self, subtitle: Option<(String, Text)>, location: Vec3, range: f32, looping: bool, now: i64) {
        let Some((key, text)) = subtitle else { return };
        if let Some(s) = self.subtitles.iter_mut().find(|s| s.key == key) {
            s.played.retain(|p| !same_location(location, p.location));
            s.played.push(PlayedAt { location, time: now });
            return;
        }
        self.subtitles.push(Subtitle { key, text, range, played: vec![PlayedAt { location, time: now }], looping });
    }

    /// `extractRenderState` at `now`: starts or stops listening as the option says, forgets old
    /// plays and lays the captions out at the bottom right.
    pub fn extract(&mut self, gui: &mut Gui, font: &dyn Metrics, f: &SubtitleFrame, now: i64) {
        self.listening = f.show;
        if !self.listening {
            return;
        }
        let (p, fw, u) = (f.position, f.forward, f.up);
        let right = [fw[1] * u[2] - fw[2] * u[1], fw[2] * u[0] - fw[0] * u[2], fw[0] * u[1] - fw[1] * u[0]];
        let mut audible = Vec::new();
        let mut width = 0;
        for (i, s) in self.subtitles.iter_mut().enumerate() {
            let max_age = 3000.0 * f.multiplier;
            s.played.retain(|at| ((now - at.time) as f64) <= max_age);
            if s.audible_from(p) && !s.played.is_empty() {
                audible.push(i);
                width = width.max(font.width(&s.text));
            }
        }
        if audible.is_empty() {
            return;
        }
        let w = |s: &str| font.width(&Text::literal(s));
        width += w("<") + w(" ") + w(">") + w(" ");
        gui.next_stratum();
        let mut row = 0;
        for i in audible {
            let s = &self.subtitles[i];
            let Some(closest) = s.best_candidate(p) else { continue };
            let d = [closest.location[0] - p[0], closest.location[1] - p[1], closest.location[2] - p[2]];
            let length = distance_sqr(d, [0.0; 3]).sqrt();
            // Vec3.normalize.
            let d = if length < f64::from(1.0e-5_f32) { [0.0; 3] } else { d.map(|v| v / length) };
            let dot = |a: Vec3| a[0] * d[0] + a[1] * d[1] + a[2] * d[2];
            let (rightness, in_view) = (dot(right), dot(fw) > 0.5);
            let (half_width, half_height) = (width / 2, 9 / 2);
            let text_width = font.width(&s.text);
            let b = s.brightness(closest, f.multiplier, now) as u32;
            gui.push(vec![
                PoseOp::Translate(gui.width as f32 - half_width as f32 * 1.0 - 2.0, (gui.height - 35) as f32 - (row * 10) as f32 * 1.0),
                PoseOp::Scale(1.0, 1.0),
            ]);
            gui.fill(-half_width - 1, -half_height - 1, half_width + 1, half_height + 1, f.background);
            let color = argb(255, b, b, b);
            if !in_view {
                if rightness > 0.0 {
                    gui.text(">", half_width - w(">"), -half_height, color, true);
                } else if rightness < 0.0 {
                    gui.text("<", -half_width, -half_height, color, true);
                }
            }
            gui.text(s.text.clone(), -text_width / 2, -half_height, color, true);
            gui.pop();
            row += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java::gui::Fixed;

    fn sub(t: &str) -> Option<(String, Text)> {
        Some((t.to_owned(), Text::literal(t)))
    }

    #[test]
    fn candidates_and_brightness() {
        let mut o = SubtitleOverlay::default();
        o.on_play(sub("a"), [0.0, 0.0, 3.0], 16.0, false, 1000);
        let s = &o.subtitles[0];
        assert_eq!(s.brightness(s.played[0], 1.0, 1000), 255);
        assert_eq!(s.brightness(s.played[0], 1.0, 2500), 165);
        assert_eq!(s.brightness(s.played[0], 2.0, 2500), 210);
        assert_eq!(s.brightness(s.played[0], 1.0, 9000), 75);
        o.on_play(sub("a"), [0.0, 0.0, 5.0], 16.0, false, 1000);
        o.on_play(sub("a"), [0.0, 0.0, 40.0], 16.0, false, 1000);
        o.on_play(None, [0.0; 3], 16.0, false, 1000);
        assert_eq!(o.subtitles.len(), 1);
        // Equally recent plays: the farthest in range wins.
        assert_eq!(o.subtitles[0].best_candidate([0.0; 3]).unwrap().location, [0.0, 0.0, 5.0]);
    }

    #[test]
    fn draws_only_while_listening() {
        let mut o = SubtitleOverlay::default();
        let mut frame = SubtitleFrame { show: true, position: [0.0; 3], forward: [0.0, 0.0, 1.0], up: [0.0, 1.0, 0.0], multiplier: 1.0, background: 0xCC00_0000 };
        let mut gui = Gui::new(320, 240);
        o.extract(&mut gui, &Fixed(6.0), &frame, 0);
        o.on_play(sub("Door"), [5.0, 0.0, 0.0], 16.0, false, 0);
        o.extract(&mut gui, &Fixed(6.0), &frame, 100);
        // Stratum, push, fill, arrow, text, pop.
        assert_eq!(gui.draws.len(), 6);
        frame.show = false;
        o.extract(&mut gui, &Fixed(6.0), &frame, 200);
        assert!(!o.listening());
        assert_eq!(gui.draws.len(), 6);
    }
}
