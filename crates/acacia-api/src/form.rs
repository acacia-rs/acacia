//! Server forms and the answers to them, mirroring acacia-bot's `forms`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A form the server showed, waiting for `bot.answerForm`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct Form {
    pub id: u32,
    /// With `§` formatting codes.
    pub title: String,
    pub body: FormBody,
}

/// What a [`Form`] asks for, by kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FormBody {
    /// Pick one button. `elements` are the labels, headers and dividers shown between buttons; `positions[i]` is
    /// how many buttons come before `elements[i]`.
    Simple { content: String, buttons: Vec<FormButton>, elements: Vec<FormElement>, positions: Vec<u32> },
    /// Two buttons: `yes` answers true.
    Modal { content: String, yes: String, no: String },
    /// Inputs to fill in.
    Custom { elements: Vec<FormElement> },
}

/// A simple form's button.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct FormButton {
    pub text: String,
    /// Texture path or URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub image: Option<String>,
}

/// One element of a custom form (an input), or a simple form's text between buttons.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FormElement {
    Label { text: String },
    Header { text: String },
    Divider,
    Input { text: String, placeholder: String, default: String },
    Toggle { text: String, default: bool },
    Slider { text: String, min: f64, max: f64, step: f64, default: f64 },
    #[serde(rename_all = "camelCase")]
    StepSlider { text: String, steps: Vec<String>, default: u32 },
    Dropdown { text: String, options: Vec<String>, default: u32 },
}

/// An answer to an open form; one that does not fit the form is refused before anything is sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FormAnswer {
    /// Simple form: the button index (buttons only, not labels or headers).
    Button { index: u32 },
    /// Modal form.
    Modal { yes: bool },
    /// Custom form: one value per input element, in order.
    Custom { values: Vec<FormValue> },
    /// The X button.
    Close,
}

/// A custom form answer's value for one input element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum FormValue {
    Text(String),
    Toggle(bool),
    Slider(f64),
    /// A dropdown option or step-slider step.
    Choice(u32),
}
