//! Server forms (`ModalFormRequest`): parsed into [`Form`]s, tracked while open, answered with
//! [`crate::Bot::answer_form`]. Formats: docs/research/forms.md.

mod actions;
mod parse;
mod reply;

use acacia_client::proto::packets::{ClientboundCloseForm, ModalFormRequest};
use acacia_client::proto::{DecodeError, Packet, RawPacket};
use tokio::time::Instant;

pub use reply::{FormReply, FormValue};

#[derive(Debug, Clone, PartialEq)]
pub struct Form {
    pub id: u32,
    /// With `§` formatting codes.
    pub title: String,
    pub kind: FormKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FormKind {
    /// Pick one button (`ActionFormData`). `elements` holds the labels, headers and dividers shown
    /// between buttons on newer servers; `buttons` is what a reply indexes.
    Simple { content: String, buttons: Vec<Button>, elements: Vec<Element> },
    /// Two buttons (`MessageFormData`): `yes` answers true, `no` false.
    Modal { content: String, yes: String, no: String },
    /// Inputs to fill in (`ModalFormData`).
    Custom { elements: Vec<Element> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Button {
    pub text: String,
    /// Texture path or URL.
    pub image: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    Label { text: String },
    Header { text: String },
    Divider,
    Input { text: String, placeholder: String, default: String },
    Toggle { text: String, default: bool },
    Slider { text: String, min: f64, max: f64, step: f64, default: f64 },
    StepSlider { text: String, steps: Vec<String>, default: usize },
    Dropdown { text: String, options: Vec<String>, default: usize },
}

impl Element {
    /// Whether the element takes a value in a custom form reply.
    pub fn is_input(&self) -> bool {
        !matches!(self, Element::Label { .. } | Element::Header { .. } | Element::Divider)
    }
}

impl Form {
    /// How many fields a custom form asks for (sizes the reading delay).
    pub(crate) fn inputs(&self) -> usize {
        match &self.kind {
            FormKind::Custom { elements } => elements.iter().filter(|e| e.is_input()).count(),
            _ => 0,
        }
    }

    /// The text a player reads before answering (sizes the reading delay).
    pub(crate) fn text_len(&self) -> usize {
        let elements = |e: &[Element]| -> usize {
            e.iter()
                .map(|e| match e {
                    Element::Label { text } | Element::Header { text } => text.len(),
                    Element::Divider => 0,
                    Element::Input { text, .. }
                    | Element::Toggle { text, .. }
                    | Element::Slider { text, .. }
                    | Element::StepSlider { text, .. }
                    | Element::Dropdown { text, .. } => text.len(),
                })
                .sum()
        };
        self.title.len()
            + match &self.kind {
                FormKind::Simple { content, buttons, elements: e } => {
                    content.len() + buttons.iter().map(|b| b.text.len()).sum::<usize>() + elements(e)
                }
                FormKind::Modal { content, yes, no } => content.len() + yes.len() + no.len(),
                FormKind::Custom { elements: e } => elements(e),
            }
    }
}

/// Forms the server has shown and the bot hasn't answered yet.
#[derive(Debug, Default)]
pub struct Forms {
    open: Vec<(Form, Instant)>,
}

impl Forms {
    pub const PACKETS: &'static [u32] = &[ModalFormRequest::ID, ClientboundCloseForm::ID];

    pub fn open(&self) -> impl Iterator<Item = &Form> {
        self.open.iter().map(|(f, _)| f)
    }

    pub fn get(&self, id: u32) -> Option<&Form> {
        self.open().find(|f| f.id == id)
    }

    /// The most recently shown open form.
    pub fn latest(&self) -> Option<&Form> {
        self.open.last().map(|(f, _)| f)
    }

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        match packet.id {
            ModalFormRequest::ID => {
                let p: ModalFormRequest = packet.decode()?;
                match parse::form(p.form_id, &p.data) {
                    Some(form) => {
                        self.remove(form.id);
                        self.open.push((form, Instant::now()));
                    }
                    None => tracing::debug!(id = p.form_id, "unparseable form"),
                }
            }
            ClientboundCloseForm::ID => self.open.clear(),
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn shown_at(&self, id: u32) -> Option<Instant> {
        self.open.iter().find(|(f, _)| f.id == id).map(|(_, t)| *t)
    }

    pub(crate) fn remove(&mut self, id: u32) {
        self.open.retain(|(f, _)| f.id != id);
    }
}
