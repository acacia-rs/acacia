//! Every data-channel message is `[u8 fragments still to follow][payload]`; 0 ends a message.

use bytes::Bytes;

use crate::Error;

/// Splits `msg` into framed fragments of at most `max_message` bytes each.
pub(crate) fn fragments(msg: &[u8], max_message: usize) -> Result<impl Iterator<Item = Vec<u8>> + '_, Error> {
    let chunk = max_message.saturating_sub(1).max(1);
    let count = msg.len().div_ceil(chunk).max(1);
    let last = u8::try_from(count - 1).map_err(|_| Error::Framing("message needs more than 256 fragments"))?;
    let mut chunks = msg.chunks(chunk);
    Ok((0..=last).rev().map(move |remaining| {
        let payload = chunks.next().unwrap_or_default();
        let mut frame = Vec::with_capacity(payload.len() + 1);
        frame.push(remaining);
        frame.extend_from_slice(payload);
        frame
    }))
}

#[derive(Default)]
pub(crate) struct Reassembler {
    buf: Vec<u8>,
    /// Fragments still expected after the last one received; None between messages.
    remaining: Option<u8>,
}

impl Reassembler {
    pub fn push(&mut self, frame: &[u8]) -> Result<Option<Bytes>, Error> {
        let [remaining, payload @ ..] = frame else {
            return Err(Error::Framing("empty message"));
        };
        if let Some(expected) = self.remaining
            && expected.checked_sub(1) != Some(*remaining)
        {
            return Err(Error::Framing("fragment count out of sequence"));
        }
        self.buf.extend_from_slice(payload);
        if *remaining > 0 {
            self.remaining = Some(*remaining);
            return Ok(None);
        }
        self.remaining = None;
        Ok(Some(Bytes::from(std::mem::take(&mut self.buf))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_single_and_split_messages() {
        for len in [1usize, 9, 10, 25] {
            let msg: Vec<u8> = (0..len as u8).collect();
            let frames: Vec<_> = fragments(&msg, 10).unwrap().collect();
            assert_eq!(frames.len(), len.div_ceil(9));
            assert_eq!(frames.last().unwrap()[0], 0);
            let mut r = Reassembler::default();
            let mut out = None;
            for f in &frames {
                out = r.push(f).unwrap();
            }
            assert_eq!(out.as_deref(), Some(&msg[..]));
        }
    }

    #[test]
    fn rejects_out_of_sequence_and_too_many() {
        let mut r = Reassembler::default();
        assert!(r.push(&[2, 1]).unwrap().is_none());
        assert!(r.push(&[0, 1]).is_err());
        assert!(fragments(&[0; 300], 2).is_err());
        assert!(Reassembler::default().push(&[]).is_err());
    }
}
