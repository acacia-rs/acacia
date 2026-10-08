use std::time::Duration;

use acacia_client::proto::packets::{ModalFormRequest, ModalFormResponse};
use acacia_client::proto::Packet;

use super::reply::{cancel, response};
use super::{Form, FormReply};
use crate::human;
use crate::{ActionError, Bot, BotEvent};

impl Bot {
    /// Answers open form `id` after a human reading delay counted from when it was shown. A
    /// reply that doesn't fit the form (button out of range, wrong value types) is refused
    /// before anything is sent.
    pub async fn answer_form(&mut self, id: u32, reply: FormReply) -> Result<(), ActionError> {
        let form = self.open_form(id)?;
        if let FormReply::Close = reply {
            return self.close_form(id).await;
        }
        let packet = response(&form, &reply)?;
        let read = self.human.reading(form.text_len(), form.inputs());
        self.after_shown(id, read).await?;
        self.send_form_response(id, &packet)
    }

    /// Answers open form `id` at once: for a person at the keyboard, whose delay is their own.
    pub fn answer_form_now(&mut self, id: u32, reply: FormReply) -> Result<(), ActionError> {
        let form = self.open_form(id)?;
        let packet = match reply {
            FormReply::Close => cancel(id),
            reply => response(&form, &reply)?,
        };
        self.send_form_response(id, &packet)
    }

    /// Closes open form `id` (the X button) after a short delay.
    pub async fn close_form(&mut self, id: u32) -> Result<(), ActionError> {
        self.open_form(id)?;
        let delay = self.human.between(human::CLICK);
        self.after_shown(id, delay).await?;
        self.send_form_response(id, &cancel(id))
    }

    /// Waits for the next form the server shows (one already open doesn't count). The form is
    /// taken out of the event queue, so it isn't returned by [`Bot::next`] as well.
    pub async fn wait_form(&mut self, timeout: Duration) -> Result<Form, ActionError> {
        let id = self
            .wait_until(timeout, |_, p| (p.id == ModalFormRequest::ID).then(|| p.decode::<ModalFormRequest>().ok()).flatten())
            .await?
            .form_id;
        self.pending.retain(|e| !matches!(e, BotEvent::Form(f) if f.id == id));
        self.open_form(id)
    }

    fn open_form(&self, id: u32) -> Result<Form, ActionError> {
        self.state.forms.get(id).cloned().ok_or_else(|| ActionError::NotPossible(format!("form {id} is not open")))
    }

    /// Waits until `delay` has passed since form `id` was shown.
    async fn after_shown(&mut self, id: u32, delay: Duration) -> Result<(), ActionError> {
        let shown = self.state.forms.shown_at(id).unwrap_or_else(tokio::time::Instant::now);
        let left = (shown + delay).saturating_duration_since(tokio::time::Instant::now());
        self.pause(left).await?;
        // The server may have closed it meanwhile (ClientboundCloseForm, or a disconnect).
        self.open_form(id).map(drop)
    }

    fn send_form_response(&mut self, id: u32, packet: &ModalFormResponse) -> Result<(), ActionError> {
        self.state.forms.remove(id);
        if self.client.send(packet) { Ok(()) } else { Err(ActionError::Disconnected) }
    }
}
