//! Form replies encoded as the vanilla client sends them (docs/research/forms.md): the JSON value
//! plus a trailing newline, `null` in custom-form slots of labels, headers and dividers.

use acacia_client::proto::packets::{ModalFormResponse, ModalFormResponseContent, ModalFormResponseContentCancelReason};
use serde_json::Value;

use super::{Element, Form, FormKind};
use crate::ActionError;

#[derive(Debug, Clone, PartialEq)]
pub enum FormReply {
    /// Simple form: the button index (buttons only, not labels or headers).
    Button(usize),
    /// Modal form: true for `yes` (button1).
    Modal(bool),
    /// Custom form: one value per input element, in order (labels, headers and dividers are skipped).
    Custom(Vec<FormValue>),
    /// The X button.
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FormValue {
    Text(String),
    Toggle(bool),
    Slider(f64),
    /// Dropdown option or step-slider step.
    Choice(usize),
}

impl Form {
    /// The custom form's default values, ready to edit and send as [`FormReply::Custom`].
    pub fn defaults(&self) -> Vec<FormValue> {
        let FormKind::Custom { elements } = &self.kind else { return Vec::new() };
        elements
            .iter()
            .filter_map(|e| match e {
                Element::Input { default, .. } => Some(FormValue::Text(default.clone())),
                Element::Toggle { default, .. } => Some(FormValue::Toggle(*default)),
                Element::Slider { default, .. } => Some(FormValue::Slider(*default)),
                Element::StepSlider { default, .. } | Element::Dropdown { default, .. } => Some(FormValue::Choice(*default)),
                Element::Label { .. } | Element::Header { .. } | Element::Divider => None,
            })
            .collect()
    }
}

pub(super) fn response(form: &Form, reply: &FormReply) -> Result<ModalFormResponse, ActionError> {
    let data = match (&form.kind, reply) {
        (FormKind::Simple { buttons, .. }, FormReply::Button(i)) if *i < buttons.len() => i.to_string(),
        (FormKind::Modal { .. }, FormReply::Modal(yes)) => yes.to_string(),
        (FormKind::Custom { elements }, FormReply::Custom(values)) => custom(elements, values)?,
        _ => return Err(ActionError::NotPossible(format!("{reply:?} does not fit form {}", form.id))),
    };
    Ok(ModalFormResponse { form_id: form.id, has_response_data: true, data: Some(data + "\n"), has_cancel_reason: false, content: None })
}

pub(super) fn cancel(id: u32) -> ModalFormResponse {
    ModalFormResponse {
        form_id: id,
        has_response_data: false,
        data: None,
        has_cancel_reason: true,
        content: Some(ModalFormResponseContent { cancel_reason: ModalFormResponseContentCancelReason::Closed }),
    }
}

fn custom(elements: &[Element], values: &[FormValue]) -> Result<String, ActionError> {
    let mismatch = |what: &str| ActionError::NotPossible(format!("custom form reply: {what}"));
    let mut values = values.iter();
    let mut slots = Vec::with_capacity(elements.len());
    for element in elements {
        if !element.is_input() {
            slots.push(Value::Null);
            continue;
        }
        let value = values.next().ok_or_else(|| mismatch("too few values"))?;
        slots.push(match (element, value) {
            (Element::Input { .. }, FormValue::Text(s)) => Value::String(s.clone()),
            (Element::Toggle { .. }, FormValue::Toggle(b)) => Value::Bool(*b),
            (Element::Slider { min, max, .. }, FormValue::Slider(x)) if (*min..=*max).contains(x) => slider(*x),
            (Element::StepSlider { steps: o, .. } | Element::Dropdown { options: o, .. }, FormValue::Choice(i)) if *i < o.len() => {
                Value::from(*i)
            }
            _ => return Err(mismatch(&format!("{value:?} does not fit {element:?}"))),
        });
    }
    if values.next().is_some() {
        return Err(mismatch("too many values"));
    }
    Ok(Value::Array(slots).to_string())
}

/// A whole value is written as an integer (vanilla sent `5` for a slider at 5, step 1.0); a
/// fractional one as a float (vanilla's spelling of those is unknown).
fn slider(x: f64) -> Value {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        return Value::from(x as i64);
    }
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

#[cfg(test)]
mod tests;
