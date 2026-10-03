mod recv;
mod send;

pub use recv::RecvError;
pub(crate) use recv::RecvState;
pub(crate) use send::SendQueue;

pub const CHANNELS: usize = 32;

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use bytes::Bytes;

    use super::*;
    use crate::wire::datagram::{decode_acks, Frame, Reliability, DATAGRAM_HEADER_LEN};
    use crate::wire::Reader;

    const MTU_PAYLOAD: usize = 1400 - 28;

    fn deliver(recv: &mut RecvState, datagram: &Bytes) {
        let mut r = Reader::new(datagram);
        r.u8().unwrap();
        recv.on_datagram(r.u24_le().unwrap());
        while r.remaining() > 0 {
            recv.on_frame(Frame::decode(&mut r, datagram).unwrap()).unwrap();
        }
    }

    fn drain(send: &mut SendQueue, now: Instant) -> Vec<Bytes> {
        std::iter::from_fn(|| send.pack(now, MTU_PAYLOAD)).collect()
    }

    fn body(i: usize, len: usize) -> Bytes {
        (0..len).map(|j| (i * 31 + j) as u8).collect::<Vec<_>>().into()
    }

    #[test]
    fn ordered_split_messages_survive_reordering_and_duplicates() {
        let now = Instant::now();
        let mut send = SendQueue::new();
        let msgs: Vec<Bytes> = (0..20).map(|i| body(i, if i % 3 == 0 { 5000 } else { 40 })).collect();
        for m in &msgs {
            send.push(m.clone(), Reliability::ReliableOrdered, 0, MTU_PAYLOAD);
        }
        let mut datagrams = drain(&mut send, now);
        assert!(datagrams.iter().all(|d| d.len() <= MTU_PAYLOAD));
        datagrams.reverse();
        let dup = datagrams[2].clone();
        datagrams.push(dup);

        let mut recv = RecvState::default();
        for d in &datagrams {
            deliver(&mut recv, d);
        }
        assert_eq!(recv.ready.drain(..).collect::<Vec<_>>(), msgs);
    }

    #[test]
    fn nacked_datagrams_are_resent_and_acks_clear_in_flight() {
        let now = Instant::now();
        let mut send = SendQueue::new();
        for i in 0..5 {
            send.push(body(i, 1000), Reliability::ReliableOrdered, 0, MTU_PAYLOAD);
        }
        let first = drain(&mut send, now);
        assert_eq!(first.len(), 5);

        let mut recv = RecvState::default();
        for (i, d) in first.iter().enumerate() {
            if i != 1 {
                deliver(&mut recv, d);
            }
        }
        assert_eq!(recv.ready.len(), 1, "messages after the gap wait for it");

        let mut ranges = vec![];
        while let Some(ack) = recv.poll_ack(MTU_PAYLOAD) {
            let nack = ack[0] & crate::wire::datagram::FLAG_NACK != 0;
            decode_acks(&ack, |a, b| ranges.push((nack, a, b))).unwrap();
        }
        for &(nack, a, b) in &ranges {
            if nack { send.on_nack(a, b) } else { send.on_ack(now, a, b) }
        }
        assert_eq!(ranges.iter().filter(|r| r.0).count(), 1);

        for d in drain(&mut send, now) {
            deliver(&mut recv, &d);
        }
        assert_eq!(recv.ready.len(), 5);
    }

    #[test]
    fn unacked_datagrams_resend_after_rto() {
        let now = Instant::now();
        let mut send = SendQueue::new();
        send.push(body(0, 10), Reliability::Reliable, 0, MTU_PAYLOAD);
        send.push(body(1, 10), Reliability::Unreliable, 0, MTU_PAYLOAD);
        assert_eq!(drain(&mut send, now).len(), 1);
        let deadline = send.next_resend().unwrap();
        send.on_timeout(deadline - Duration::from_millis(1));
        assert!(send.pack(now, MTU_PAYLOAD).is_none());
        send.on_timeout(deadline);
        let resent = drain(&mut send, deadline);
        assert_eq!(resent.len(), 1);
        assert_eq!(resent[0].len(), DATAGRAM_HEADER_LEN + 3 + 3 + 10, "only the reliable frame is resent");
    }
}
