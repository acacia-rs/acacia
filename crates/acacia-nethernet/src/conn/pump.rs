use std::time::Instant;

use str0m::{IceConnectionState, Output};

use super::{Connection, Event, Transmit};
use crate::frame::fragments;

/// str0m buffers at most 128 KiB across streams (`MAX_BUFFERED_ACROSS_STREAMS`); a bigger frame
/// never fits. Vanilla splits at max-message-size, so only messages over 128 KiB differ.
const MAX_FRAME: usize = 128 * 1024;

impl Connection {
    /// Runs str0m until it waits for input, collecting its output.
    pub(super) fn drive(&mut self, now: Instant) {
        if self.closed {
            return;
        }
        self.write_outbound();
        loop {
            let output = match self.rtc.poll_output() {
                Ok(o) => o,
                Err(e) => return self.close_with(format!("poll: {e}")),
            };
            match output {
                Output::Timeout(t) => {
                    self.timeout = Some(t.max(now));
                    return;
                }
                Output::Transmit(t) => self.transmits.push_back(Transmit {
                    source: t.source,
                    destination: t.destination,
                    contents: t.contents.to_vec(),
                }),
                Output::Event(e) => self.handle_event(e),
            }
            if self.closed {
                return;
            }
        }
    }

    fn handle_event(&mut self, event: str0m::Event) {
        use str0m::Event as E;
        if !matches!(event, E::ChannelData(_) | E::ChannelBufferedAmountLow(_)) {
            tracing::debug!(?event, "rtc");
        }
        match event {
            E::ChannelOpen(id, _) => {
                let was_open = self.is_open();
                if id == self.reliable {
                    self.open[0] = true;
                } else if id == self.unreliable {
                    self.open[1] = true;
                }
                if !was_open && self.is_open() {
                    self.events.push_back(Event::Open);
                    self.write_outbound();
                }
            }
            // Unreliable messages are never split, so they bypass the reliable reassembler.
            E::ChannelData(d) if d.id == self.unreliable => match d.data.split_first() {
                Some((0, msg)) => self.messages.push_back(bytes::Bytes::copy_from_slice(msg)),
                _ => tracing::trace!("dropping fragmented unreliable message"),
            },
            E::ChannelData(d) => match self.reassembler.push(&d.data) {
                Ok(Some(msg)) => self.messages.push_back(msg),
                Ok(None) => {}
                Err(e) => self.close_with(e.to_string()),
            },
            E::ChannelBufferedAmountLow(_) => self.write_outbound(),
            E::ChannelClose(id) => self.close_with(format!("data channel {id:?} closed")),
            E::IceConnectionStateChange(IceConnectionState::Disconnected) => self.close_with("ICE disconnected".into()),
            _ => {}
        }
    }

    /// Frames queued messages and writes until the reliable channel's buffer is full.
    fn write_outbound(&mut self) {
        if !self.is_open() {
            return;
        }
        while let Some(msg) = self.unsent.pop_front() {
            match fragments(&msg, self.max_message.min(MAX_FRAME)) {
                Ok(frames) => self.outbound.extend(frames),
                Err(e) => return self.close_with(e.to_string()),
            }
        }
        let Some(mut channel) = self.rtc.channel(self.reliable) else { return };
        let mut failed = None;
        while let Some(frame) = self.outbound.front() {
            match channel.write(true, frame) {
                Ok(true) => {
                    self.outbound.pop_front();
                }
                Ok(false) => break,
                Err(e) => {
                    failed = Some(format!("write: {e}"));
                    break;
                }
            }
        }
        if let Some(reason) = failed {
            self.close_with(reason);
        }
    }

    pub(super) fn close_with(&mut self, reason: String) {
        if !self.closed {
            tracing::debug!(%reason, "nethernet closed");
            self.closed = true;
            self.timeout = None;
            self.events.push_back(Event::Closed(reason));
        }
    }
}
