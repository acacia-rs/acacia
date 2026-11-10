//! The open dialog and pressing its buttons as a vanilla `DialogScreen` does: a command goes out as an unsigned
//! `ChatCommand`, a custom action as `CustomClickAction`, and the inputs fill `$(key)` templates or the payload.

use acacia_java_proto::dialog::{Action, Button, Control, CustomClickAction, Dialog, Input};
use acacia_java_proto::ids::play::serverbound as to;
use acacia_java_proto::tag::Tag;

use crate::frame::Error;
use crate::join::Client;

/// A value for one dialog input, in the dialog's input order.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    Bool(bool),
    /// The index of the chosen option.
    Option(usize),
    Number(f32),
}

/// What pressing a button does.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Command(String),
    Custom { id: String, payload: Option<Tag> },
    /// Another dialog opens in this one's place; nothing is sent.
    Open(Dialog),
    Nothing,
}

/// The effect of `button` with `values` filled into `inputs`; `values` must match `inputs` one for one.
pub fn effect(inputs: &[Input], button: &Button, values: &[Value]) -> Effect {
    match &button.action {
        None | Some(Action::ClientOnly) => Effect::Nothing,
        Some(Action::RunCommand(command)) => Effect::Command(command.clone()),
        Some(Action::DynamicRunCommand(template)) => Effect::Command(fill(template, inputs, values)),
        Some(Action::Custom { id, payload }) => Effect::Custom { id: id.clone(), payload: payload.clone() },
        Some(Action::DynamicCustom { id, additions }) => {
            let mut entries = match additions {
                Some(Tag::Compound(entries)) => entries.clone(),
                _ => Vec::new(),
            };
            for (input, value) in inputs.iter().zip(values) {
                entries.retain(|(k, _)| *k != input.key);
                entries.push((input.key.clone(), tag(input, value)));
            }
            Effect::Custom { id: id.clone(), payload: Some(Tag::Compound(entries)) }
        }
        Some(Action::ShowDialog(dialog)) => Effect::Open((**dialog).clone()),
    }
}

/// The values a dialog opens with.
pub fn initial_values(inputs: &[Input]) -> Vec<Value> {
    inputs
        .iter()
        .map(|i| match &i.control {
            Control::Text { initial, .. } => Value::Text(initial.clone()),
            Control::Boolean { initial, .. } => Value::Bool(*initial),
            Control::SingleOption { initial, .. } => Value::Option(*initial),
            Control::NumberRange { initial, .. } => Value::Number(*initial),
        })
        .collect()
}

/// `NumberRangeInput`'s snapping: whole steps from `initial`, pulled back inside the range.
pub fn snap(value: f32, start: f32, end: f32, initial: f32, step: Option<f32>) -> f32 {
    let (lo, hi) = (start.min(end), start.max(end));
    let Some(step) = step.filter(|s| *s > 0.0) else { return value.clamp(lo, hi) };
    let mut v = initial + ((value - initial) / step).round() * step;
    if v > hi {
        v -= step;
    } else if v < lo {
        v += step;
    }
    v
}

fn template_text(input: &Input, value: &Value) -> String {
    match (value, &input.control) {
        (Value::Text(s), _) => s.chars().flat_map(|c| if matches!(c, '"' | '\'' | '\\') { vec!['\\', c] } else { vec![c] }).collect(),
        (Value::Bool(b), Control::Boolean { on_true, on_false, .. }) => if *b { on_true.clone() } else { on_false.clone() },
        (Value::Bool(b), _) => b.to_string(),
        (Value::Option(i), Control::SingleOption { options, .. }) => options.get(*i).map(|(id, _)| id.clone()).unwrap_or_default(),
        (Value::Option(i), _) => i.to_string(),
        (Value::Number(n), _) if n.fract() == 0.0 => format!("{}", *n as i64),
        (Value::Number(n), _) => format!("{n:?}"),
    }
}

fn tag(input: &Input, value: &Value) -> Tag {
    match (value, &input.control) {
        (Value::Text(s), _) => Tag::String(s.clone()),
        (Value::Bool(b), _) => Tag::Byte(i8::from(*b)),
        (Value::Option(i), Control::SingleOption { options, .. }) => Tag::String(options.get(*i).map(|(id, _)| id.clone()).unwrap_or_default()),
        (Value::Option(i), _) => Tag::Int(*i as i32),
        (Value::Number(n), _) => Tag::Float(*n),
    }
}

/// `$(key)` macros replaced by the inputs' template text; unknown keys become empty.
fn fill(template: &str, inputs: &[Input], values: &[Value]) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("$(") {
        out.push_str(&rest[..start]);
        let Some(len) = rest[start + 2..].find(')') else {
            rest = &rest[start..];
            break;
        };
        let key = &rest[start + 2..start + 2 + len];
        if let Some((input, value)) = inputs.iter().zip(values).find(|(i, _)| i.key == key) {
            out.push_str(&template_text(input, value));
        }
        rest = &rest[start + 3 + len..];
    }
    out + rest
}

impl Client {
    /// Sends what `effect` sends; a command goes without its leading `/`, unsigned, as from a dialog.
    pub fn send_effect(&self, effect: &Effect) -> Result<(), Error> {
        match effect {
            Effect::Command(command) => self.command(command),
            Effect::Custom { id, payload } => self.send(to::CUSTOM_CLICK_ACTION, &CustomClickAction { id: id.clone(), payload: payload.clone() }),
            Effect::Open(_) | Effect::Nothing => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "dialog_tests.rs"]
mod tests;
