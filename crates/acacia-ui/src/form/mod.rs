//! Server forms (Bedrock's `ModalFormRequest`): the action form (pick a button), the modal form
//! (yes or no) and the custom form (fill in widgets, submit). The Bedrock look draws the 225×200
//! dialog of `server_form.json`; the Java look lays the form out as Java's own server dialogs
//! (`DialogScreen`). Both from research/forms-*.md in the workspace.

mod bedrock;
mod java;

use std::time::Instant;

use crate::draw::DrawList;
use crate::input::{Input, Key};
use crate::theme::Theme;
use crate::widget::panel::{Panel, Response};
use crate::widget::{Value, Widget, Widgets};

/// What the server sent, as widgets.
#[derive(Debug, Clone, PartialEq)]
pub enum Spec {
    /// `items` are buttons, text, headers and dividers, in order.
    Action { content: String, items: Vec<Widget> },
    Modal { content: String, yes: String, no: String },
    Custom { elements: Vec<Widget> },
}

/// How the player answered.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The n-th button of an action form (counting buttons only).
    Button(usize),
    Modal(bool),
    /// A custom form's input values, in order.
    Submit(Vec<Value>),
    Close,
}

/// Buttons that answer the form rather than belong to its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Yes,
    No,
    Submit,
}

/// A form laid out for one screen size: the panel holds the content widgets, then the action
/// buttons.
pub struct FormView {
    title: String,
    panel: Panel,
    content: usize,
    shape: Shape,
    actions: Vec<Action>,
    chrome: Chrome,
    size: [f32; 2],
    mouse: [f32; 2],
    close_held: bool,
}

/// What the layouts need to know of the form's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shape {
    /// The first content widget is the body text (action and modal forms).
    body: bool,
    /// A custom form: text widgets are labels, the column is wider.
    custom: bool,
}

/// Look-specific frame around the panel.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Chrome {
    /// The dialog's top-left.
    Bedrock([f32; 2]),
    Java,
}

/// The submit button's text (Bedrock's `#submit_text`, Java's `gui.done`).
const SUBMIT: &str = "Submit";

impl FormView {
    pub fn new(title: &str, spec: Spec, theme: &Theme, size: [f32; 2]) -> FormView {
        let shape = Shape { body: matches!(&spec, Spec::Action { content, .. } | Spec::Modal { content, .. } if !content.is_empty()), custom: matches!(spec, Spec::Custom { .. }) };
        let (content, actions) = match spec {
            Spec::Action { content, items } => (text_then(content, items), Vec::new()),
            Spec::Modal { content, yes, no } => (text_then(content, Vec::new()), vec![(Action::Yes, yes), (Action::No, no)]),
            Spec::Custom { elements } => (elements, vec![(Action::Submit, SUBMIT.to_owned())]),
        };
        let buttons = actions.iter().map(|(_, text)| Widget::Button { text: text.clone(), image: None }).collect();
        let mut view = FormView {
            title: title.to_owned(),
            panel: Panel::new(Vec::new(), [0.0; 4], 0.0),
            content: content.len(),
            shape,
            actions: actions.into_iter().map(|(a, _)| a).collect(),
            chrome: Chrome::Java,
            size,
            mouse: [-1.0; 2],
            close_held: false,
        };
        view.lay_out(theme, content, buttons);
        view
    }

    fn lay_out(&mut self, theme: &Theme, content: Vec<Widget>, buttons: Vec<Widget>) {
        let font = theme.font.as_ref();
        (self.panel, self.chrome) = match &theme.widgets {
            Widgets::Bedrock(kit) => {
                let (panel, origin) = bedrock::lay_out(kit, font, self.shape, content, buttons, self.size);
                (panel, Chrome::Bedrock(origin))
            }
            Widgets::Java(kit) => (java::lay_out(kit, font, content, buttons, self.size), Chrome::Java),
        };
    }

    /// Lays the form out again for a new screen size, keeping what was entered.
    pub fn resize(&mut self, theme: &Theme, size: [f32; 2]) {
        if size == self.size {
            return;
        }
        self.size = size;
        let mut widgets: Vec<Widget> = std::mem::take(&mut self.panel.items).into_iter().map(|p| p.widget).collect();
        let buttons = widgets.split_off(self.content);
        self.lay_out(theme, widgets, buttons);
    }

    /// Sets the image of the n-th button of an action form (an image that finished loading).
    pub fn set_image(&mut self, button: usize, sprite: crate::atlas::Sprite) {
        let found = self.panel.items[..self.content].iter_mut().filter_map(|p| match &mut p.widget {
            Widget::Button { image, .. } => Some(image),
            _ => None,
        });
        if let Some(image) = found.into_iter().nth(button) {
            *image = Some(sprite);
        }
    }

    pub fn handle(&mut self, input: &Input, theme: &Theme) -> Option<Outcome> {
        let close = self.close_rect();
        match input {
            Input::Move(at) => self.mouse = *at,
            Input::Press(at) if close.is_some_and(|r| crate::widget::contains(r, *at)) => {
                self.close_held = true;
                return None;
            }
            Input::Release(at) if self.close_held => {
                self.close_held = false;
                return close.is_some_and(|r| crate::widget::contains(r, *at)).then_some(Outcome::Close);
            }
            _ => {}
        }
        match self.panel.handle(input, theme.widgets.skin(), theme.font.as_ref()) {
            Response::Pressed(i) => Some(self.outcome(i)),
            Response::Ignored if *input == Input::Key(Key::Escape, Default::default()) => Some(Outcome::Close),
            Response::Ignored | Response::Consumed => None,
        }
    }

    fn outcome(&self, i: usize) -> Outcome {
        if i < self.content {
            let n = self.panel.items[..i].iter().filter(|p| matches!(p.widget, Widget::Button { .. })).count();
            return Outcome::Button(n);
        }
        match self.actions[i - self.content] {
            Action::Yes => Outcome::Modal(true),
            Action::No => Outcome::Modal(false),
            Action::Submit => Outcome::Submit(self.panel.items[..self.content].iter().filter_map(|p| p.widget.value()).collect()),
        }
    }

    fn close_rect(&self) -> Option<[f32; 4]> {
        match self.chrome {
            Chrome::Bedrock(origin) => Some(bedrock::close_rect(origin)),
            Chrome::Java => None,
        }
    }

    pub fn draw(&self, list: &mut DrawList, theme: &Theme, now: Instant) {
        let font = theme.font.as_ref();
        match (&theme.widgets, self.chrome) {
            (Widgets::Bedrock(kit), Chrome::Bedrock(origin)) => {
                let close = self.close_rect().is_some_and(|r| crate::widget::contains(r, self.mouse));
                bedrock::draw_frame(list, kit, font, &self.title, origin, close, self.close_held);
            }
            (Widgets::Java(_), _) => java::draw_frame(list, theme.atlas.white(), font, &self.title, self.size),
            _ => {}
        }
        self.panel.draw(list, theme.widgets.skin(), font, now);
    }
}

/// `content` as a text widget (when there is any), then `items`.
fn text_then(content: String, items: Vec<Widget>) -> Vec<Widget> {
    let text = (!content.is_empty()).then_some(Widget::Text { text: content });
    text.into_iter().chain(items).collect()
}

#[cfg(test)]
mod tests;
