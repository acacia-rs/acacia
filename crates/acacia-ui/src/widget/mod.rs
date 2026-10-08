//! Widgets (buttons, toggles, sliders, dropdowns, text fields, text) placed on a [`panel::Panel`]
//! that scrolls and takes input. How each looks is the theme's [`Skin`]: Bedrock's `textures/ui`
//! controls (`bedrock.rs`) or Java's `sprites/widget` (`java.rs`). Measurements:
//! research/forms-bedrock-layout.md and research/forms-java-widgets.md in the workspace.

pub mod bedrock;
mod edit;
pub mod java;
pub mod panel;
#[cfg(test)]
pub(crate) mod test_theme;

pub use edit::TextEdit;

use crate::atlas::Sprite;
use crate::draw::DrawList;
use crate::font::Font;

#[derive(Debug, Clone, PartialEq)]
pub enum Widget {
    /// Wrapped text; `\n` breaks lines.
    Text { text: String },
    Header { text: String },
    Divider,
    Button { text: String, image: Option<Sprite> },
    Toggle { label: String, on: bool },
    Slider { label: String, min: f64, max: f64, step: f64, value: f64 },
    /// A slider over named steps.
    Steps { label: String, options: Vec<String>, index: usize },
    Dropdown { label: String, options: Vec<String>, index: usize },
    Input { label: String, placeholder: String, edit: TextEdit },
}

/// What an input widget holds.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    Toggle(bool),
    Number(f64),
    /// Dropdown option or step.
    Choice(usize),
}

impl Widget {
    /// Takes focus and input.
    pub fn is_interactive(&self) -> bool {
        !matches!(self, Widget::Text { .. } | Widget::Header { .. } | Widget::Divider)
    }

    /// The value an input widget holds; `None` for text and buttons.
    pub fn value(&self) -> Option<Value> {
        Some(match self {
            Widget::Toggle { on, .. } => Value::Toggle(*on),
            Widget::Slider { value, .. } => Value::Number(*value),
            Widget::Steps { index, .. } | Widget::Dropdown { index, .. } => Value::Choice(*index),
            Widget::Input { edit, .. } => Value::Text(edit.text()),
            _ => return None,
        })
    }

    /// `Label: value` for sliders and step sliders, as both games show it.
    pub fn label_with_value(&self) -> Option<String> {
        match self {
            Widget::Slider { label, value, .. } => Some(format!("{label}: {}", number(*value))),
            Widget::Steps { label, options, index } | Widget::Dropdown { label, options, index } => {
                Some(format!("{label}: {}", options.get(*index).map_or("", String::as_str)))
            }
            _ => None,
        }
    }
}

/// Integral values without a fraction, others as the shortest float (Java's `Float.toString`).
pub fn number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{}", v as f32) }
}

/// How a widget is drawn this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State {
    pub hover: bool,
    pub focus: bool,
    /// The mouse is held on it.
    pub pressed: bool,
    /// A dropdown's list is open.
    pub open: bool,
    /// The text caret is in its visible half of the blink.
    pub caret: bool,
}

/// Where the open list of a dropdown lies, and its rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListGeometry {
    pub rect: [f32; 4],
    /// The first row's top-left before scrolling, and the row pitch.
    pub first: [f32; 2],
    pub row: f32,
    pub row_width: f32,
}

/// A look's drawing of the widgets; geometry the panel needs to take input.
pub trait Skin {
    fn draw(&self, list: &mut DrawList, font: Option<&Font>, widget: &Widget, rect: [f32; 4], state: State);
    /// The part of `rect` that takes clicks.
    fn hit(&self, font: Option<&Font>, widget: &Widget, rect: [f32; 4]) -> [f32; 4];
    /// The x span the slider's value maps onto, 0 at the left.
    fn slider_span(&self, font: Option<&Font>, rect: [f32; 4]) -> [f32; 2];
    /// Dropdowns and step sliders are buttons stepping through their options (Java's
    /// `CycleButton`) rather than a list and a slider (Bedrock).
    fn choices_cycle(&self) -> bool;
    fn dropdown_list(&self, font: Option<&Font>, rect: [f32; 4], options: usize) -> ListGeometry;
    fn draw_dropdown_list(&self, list: &mut DrawList, font: Option<&Font>, geometry: ListGeometry, options: &[String], selected: usize, hover: Option<usize>, scroll: f32);
    /// The scrollbar's track beside `viewport`.
    fn scroll_track(&self, viewport: [f32; 4]) -> [f32; 4];
    fn thumb(&self, track: [f32; 4], visible: f32, content: f32, scroll: f32) -> [f32; 4];
    fn draw_scrollbar(&self, list: &mut DrawList, track: [f32; 4], thumb: [f32; 4]);
    /// GUI pixels per wheel notch.
    fn scroll_step(&self) -> f32;
}

/// A theme's widgets: which game's, so screens can lay out as that game does.
pub enum Widgets {
    Bedrock(bedrock::Kit),
    Java(java::Kit),
}

impl Widgets {
    pub fn skin(&self) -> &dyn Skin {
        match self {
            Widgets::Bedrock(kit) => kit,
            Widgets::Java(kit) => kit,
        }
    }
}

pub fn contains(rect: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= rect[0] && p[0] < rect[2] && p[1] >= rect[1] && p[1] < rect[3]
}

/// `text` wrapped to `width`, `\n` starting a new line.
pub fn lines(font: Option<&Font>, text: &str, width: f32) -> Vec<String> {
    match font {
        Some(f) => text.split('\n').flat_map(|part| f.wrap(part, width)).collect(),
        None => text.split('\n').map(str::to_owned).collect(),
    }
}

/// Runs `draw` with the clip narrowed to `rect`, then restores it.
pub(crate) fn clipped(list: &mut DrawList, rect: [f32; 4], draw: impl FnOnce(&mut DrawList)) {
    let old = list.clip;
    list.clip = Some(match old {
        Some(c) => [c[0].max(rect[0]), c[1].max(rect[1]), c[2].min(rect[2]), c[3].min(rect[3])],
        None => rect,
    });
    draw(list);
    list.clip = old;
}

/// Draws `lines` from `y` down, `pitch` apart, left-aligned at `x` or centred on it.
#[allow(clippy::too_many_arguments)]
pub fn draw_lines(list: &mut DrawList, font: Option<&Font>, lines: &[String], x: f32, y: f32, pitch: f32, centre: bool, colour: u32, shadow: bool) {
    let Some(font) = font else { return };
    for (i, line) in lines.iter().enumerate() {
        let lx = if centre { (x - font.width(line) / 2.0).floor() } else { x };
        font.draw(list, line, lx, y + i as f32 * pitch, colour, 1.0, shadow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_print_as_the_games_do() {
        assert_eq!(number(5.0), "5");
        assert_eq!(number(0.5), "0.5");
        assert_eq!(number(-2.25), "-2.25");
        let w = Widget::Slider { label: "Volume".into(), min: 0.0, max: 10.0, step: 1.0, value: 3.0 };
        assert_eq!(w.label_with_value().as_deref(), Some("Volume: 3"));
    }
}
