//! Java dialogs as [`Form`]s: a notice, multi-action or list dialog is a simple form, a confirmation a modal one,
//! and a dialog with inputs [`FormKind::Dialog`]. Answering presses a button as vanilla does
//! (`acacia_java_client::dialog`); closing runs the dialog's cancel action, as Escape does.

use acacia_java_client::dialog::{Effect, Value, effect, initial_values, snap};
use acacia_java_proto::dialog::{Control, Dialog, DialogKind, Input, Shown};

use super::{Button, Element, Form, FormKind, FormReply, FormValue, Forms};
use crate::{ActionError, Bot};

impl Forms {
    /// Mirrors the client's open dialog; returns the form when a new one opened.
    pub(crate) fn set_java(&mut self, shown: Option<&Shown>) -> Option<Form> {
        if let Some((id, _)) = self.java.take() {
            self.remove(id);
        }
        let dialog = match shown? {
            Shown::Inline(dialog) => dialog.clone(),
            Shown::Registry(n) => unknown(*n),
        };
        Some(self.open_java(dialog))
    }

    fn open_java(&mut self, dialog: Dialog) -> Form {
        self.next_java_id = self.next_java_id.wrapping_add(1);
        let form = form(self.next_java_id, &dialog);
        self.open.push((form.clone(), tokio::time::Instant::now()));
        self.java = Some((form.id, dialog));
        form
    }
}

impl Bot {
    /// [`Bot::answer_form`] on Java: refused before the reading delay if it does not fit.
    pub(super) async fn java_answer_form(&mut self, id: u32, reply: FormReply) -> Result<(), ActionError> {
        press(&self.open_dialog(id)?, &reply)?;
        let form = self.state.forms.get(id).cloned().ok_or_else(|| ActionError::NotPossible(format!("form {id} is not open")))?;
        let delay = match reply {
            FormReply::Close => self.human.between(crate::human::CLICK),
            _ => self.human.reading(form.text_len(), form.inputs()),
        };
        self.after_shown(id, delay).await?;
        self.java_answer_form_now(id, reply)
    }

    fn open_dialog(&self, id: u32) -> Result<Dialog, ActionError> {
        let open = self.state.forms.java.as_ref().filter(|(open, _)| *open == id);
        open.map(|(_, d)| d.clone()).ok_or_else(|| ActionError::NotPossible(format!("form {id} is not open")))
    }

    /// [`Bot::answer_form_now`] on Java.
    pub(super) fn java_answer_form_now(&mut self, id: u32, reply: FormReply) -> Result<(), ActionError> {
        let dialog = self.open_dialog(id)?;
        let effect = press(&dialog, &reply)?;
        let java = self.java().ok_or(ActionError::Disconnected)?.client();
        java.send_effect(&effect).map_err(|_| ActionError::Disconnected)?;
        let forms = &mut self.state.forms;
        match effect {
            Effect::Open(next) => {
                forms.set_java(None);
                forms.open_java(next);
            }
            _ if dialog.stays_open && reply != FormReply::Close => {}
            _ => {
                forms.set_java(None);
            }
        }
        Ok(())
    }
}

/// A registry dialog the bot has no data for (vanilla's own, sent by name under the known packs): only closable.
fn unknown(index: usize) -> Dialog {
    Dialog {
        kind: DialogKind::ServerLinks,
        title: format!("minecraft:dialog #{index}"),
        body: Vec::new(),
        inputs: Vec::new(),
        buttons: Vec::new(),
        cancel: None,
        can_close_with_escape: true,
        stays_open: false,
    }
}

pub(crate) fn form(id: u32, dialog: &Dialog) -> Form {
    let content = dialog.body.join("\n");
    let buttons: Vec<Button> = dialog.buttons.iter().map(|b| Button { text: b.label.clone(), image: None }).collect();
    let kind = match (dialog.kind, dialog.inputs.is_empty()) {
        (DialogKind::Confirmation, true) => {
            let label = |i: usize| buttons.get(i).map(|b| b.text.clone()).unwrap_or_default();
            FormKind::Modal { content, yes: label(0), no: label(1) }
        }
        (_, true) => FormKind::Simple { content, buttons, elements: Vec::new(), positions: Vec::new() },
        (_, false) => FormKind::Dialog { content, elements: dialog.inputs.iter().map(element).collect(), buttons },
    };
    Form { id, title: dialog.title.clone(), kind }
}

fn element(input: &Input) -> Element {
    let text = input.label.clone();
    match &input.control {
        Control::Text { initial, .. } => Element::Input { text, placeholder: String::new(), default: initial.clone() },
        Control::Boolean { initial, .. } => Element::Toggle { text, default: *initial },
        Control::SingleOption { options, initial } => Element::Dropdown { text, options: options.iter().map(|(_, shown)| shown.clone()).collect(), default: *initial },
        Control::NumberRange { start, end, initial, step } => Element::Slider {
            text,
            min: f64::from(*start),
            max: f64::from(*end),
            step: step.map_or(0.0, f64::from),
            default: f64::from(*initial),
        },
    }
}

/// What `reply` sends on `dialog`.
fn press(dialog: &Dialog, reply: &FormReply) -> Result<Effect, ActionError> {
    let refuse = || ActionError::NotPossible(format!("{reply:?} does not fit the dialog"));
    let (button, values) = match reply {
        FormReply::Close if !dialog.can_close_with_escape => return Err(ActionError::NotPossible("this dialog cannot be closed".into())),
        FormReply::Close => return Ok(dialog.cancel.as_ref().map_or(Effect::Nothing, |b| effect(&dialog.inputs, b, &initial_values(&dialog.inputs)))),
        FormReply::Button(i) => (*i, initial_values(&dialog.inputs)),
        FormReply::Modal(yes) if dialog.kind == DialogKind::Confirmation => (usize::from(!yes), initial_values(&dialog.inputs)),
        FormReply::Custom(values) => (0, values_for(&dialog.inputs, values).ok_or_else(refuse)?),
        FormReply::Dialog { button, values } => (*button, values_for(&dialog.inputs, values).ok_or_else(refuse)?),
        FormReply::Modal(_) => return Err(refuse()),
    };
    let button = dialog.buttons.get(button).ok_or_else(refuse)?;
    Ok(effect(&dialog.inputs, button, &values))
}

/// Form values as dialog values: one per input, each of the input's kind and within its limits.
fn values_for(inputs: &[Input], values: &[FormValue]) -> Option<Vec<Value>> {
    if inputs.len() != values.len() {
        return None;
    }
    inputs
        .iter()
        .zip(values)
        .map(|(input, value)| match (&input.control, value) {
            (Control::Text { max_length, .. }, FormValue::Text(s)) if s.chars().count() <= *max_length => Some(Value::Text(s.clone())),
            (Control::Boolean { .. }, FormValue::Toggle(b)) => Some(Value::Bool(*b)),
            (Control::SingleOption { options, .. }, FormValue::Choice(i)) if *i < options.len() => Some(Value::Option(*i)),
            (Control::NumberRange { start, end, initial, step }, FormValue::Slider(x)) => {
                let x = *x as f32;
                (start.min(*end)..=start.max(*end)).contains(&x).then(|| Value::Number(snap(x, *start, *end, *initial, *step)))
            }
            _ => None,
        })
        .collect()
}
