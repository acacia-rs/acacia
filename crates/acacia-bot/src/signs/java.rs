//! Java signs: the server's `OpenSignEditor` fills the same [`crate::state::Signs`] editor, and closing it sends
//! `SignUpdate` with the side's four lines (acacia-java-client `signs.rs`).

use acacia_java_client::Packet;
use acacia_java_proto::actions::OpenSignEditor;
use acacia_java_proto::ids::play::clientbound::{OPEN_SIGN_EDITOR, RESPAWN};
use acacia_java_proto::Wire;

use super::MAX_LINES;
use crate::bot::Backend;
use crate::state::SignEditor;
use crate::Bot;

impl Bot {
    pub(crate) fn on_java_sign(&mut self, packet: &Packet) {
        match packet.id {
            OPEN_SIGN_EDITOR => match OpenSignEditor::read(&mut &packet.body[..]) {
                Ok(p) => self.state.signs.editor = Some(SignEditor { position: p.pos.0, front: p.front }),
                Err(e) => tracing::debug!(error = ?e, "unreadable sign editor"),
            },
            RESPAWN => self.state.signs.editor = None,
            _ => {}
        }
    }

    /// Sends `text` as the side's lines; false off Java.
    pub(super) fn java_close_sign_editor(&mut self, pos: [i32; 3], front: bool, text: &str) -> bool {
        let Backend::Java(java) = &mut self.backend else { return false };
        let mut lines: [String; MAX_LINES] = Default::default();
        for (line, typed) in lines.iter_mut().zip(text.split('\n')) {
            *line = typed.to_owned();
        }
        if let Err(e) = java.client.update_sign(pos, front, lines) {
            tracing::debug!(error = %e, "sign update not sent");
        }
        self.state.signs.editor = None;
        true
    }
}
