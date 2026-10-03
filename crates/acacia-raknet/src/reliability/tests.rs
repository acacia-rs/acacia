use std::time::{Duration, Instant};

use bytes::Bytes;

use super::*;
use crate::wire::datagram::{decode_acks, Frame, Reliability, Split, DATAGRAM_HEADER_LEN};
use crate::wire::Reader;

const MTU_PAYLOAD: usize = 1400 - 28;

fn deliver(recv: &mut RecvState, datagram: &Bytes) {
    let mut r = Reader::new(datagram);
    r.u8().unwrap();
    recv.on_datagram(r.u24_le().unwrap());
    while r.remaining() > 0 {
        recv.on_frame(Frame::decode(&mut r, datagram).unwrap(), datagram.len()).unwrap();
    }
}

fn drain(send: &mut SendQueue, now: Instant) -> Vec<Bytes> {
    std::iter::from_fn(|| send.pack(now, MTU_PAYLOAD)).collect()
}

fn body(i: usize, len: usize) -> Bytes {
    (0..len).map(|j| (i * 31 + j) as u8).collect::<Vec<_>>().into()
}

/// A reliable ordered frame; `order_index` doubles as its reliable index.
fn ordered(order_index: u32, split: Option<Split>, len: usize) -> Frame {
    let reliability = Reliability::ReliableOrdered;
    Frame { reliability, reliable_index: order_index, sequence_index: 0, order_index, order_channel: 0, split, body: body(0, len) }
}

/// The first error from feeding `frames` to a fresh receiver with `limits`.
fn first_error(limits: RecvLimits, frames: &[Frame]) -> Option<RecvError> {
    let mut recv = RecvState::new(limits);
    frames.iter().find_map(|f| recv.on_frame(f.clone(), f.body.len()).err())
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

    let mut recv = RecvState::new(RecvLimits::SERVER);
    for d in &datagrams {
        deliver(&mut recv, d);
    }
    assert_eq!(recv.ready.drain(..).collect::<Vec<_>>(), msgs);
    assert_eq!(recv.buffered(), 0, "everything buffered was handed over");
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

    let mut recv = RecvState::new(RecvLimits::SERVER);
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

#[test]
fn a_split_costs_what_arrived_not_what_it_claims() {
    let mut recv = RecvState::new(RecvLimits::SERVER);
    for id in 0..RecvLimits::SERVER.max_splits as u16 {
        let last = Split { count: RecvLimits::SERVER.max_split_parts, id, index: RecvLimits::SERVER.max_split_parts - 1 };
        recv.on_frame(ordered(u32::from(id), Some(last), 1), 1).unwrap();
    }
    assert!(recv.buffered() < 2048, "{} bytes charged for 16 one-byte fragments", recv.buffered());
}

#[test]
fn split_abuse_is_refused() {
    let limits = RecvLimits { max_split_parts: 8, max_splits: 2, max_buffered_bytes: 1000 };
    let part = |id, count, index, order| ordered(order, Some(Split { count, id, index }), 10);
    assert_eq!(first_error(limits, &[part(0, 8, 0, 0), part(1, 8, 0, 1)]), None);
    assert_eq!(first_error(limits, &[part(0, 0, 0, 0)]), Some(RecvError::BadSplit));
    assert_eq!(first_error(limits, &[part(0, 9, 0, 0)]), Some(RecvError::BadSplit));
    assert_eq!(first_error(limits, &[part(0, 8, 8, 0)]), Some(RecvError::BadSplit));
    assert_eq!(first_error(limits, &[part(0, 8, 0, 0), part(0, 7, 1, 1)]), Some(RecvError::BadSplit), "the count changed");
    assert_eq!(first_error(limits, &[part(0, 8, 0, 0), part(1, 8, 0, 1), part(2, 8, 0, 2)]), Some(RecvError::TooManySplits));
    let big = |index| ordered(index, Some(Split { count: 8, id: 0, index }), 600);
    assert_eq!(first_error(limits, &[big(0), big(1)]), Some(RecvError::BufferFull));
}

#[test]
fn the_buffer_budget_covers_the_order_backlog_and_is_returned() {
    let limits = RecvLimits { max_buffered_bytes: 1000, ..RecvLimits::SERVER };
    let early = [ordered(1, None, 300), ordered(2, None, 300)];
    assert_eq!(first_error(limits, &[early[0].clone(), early[1].clone(), ordered(3, None, 300)]), Some(RecvError::BufferFull));

    let mut recv = RecvState::new(limits);
    for f in early {
        recv.on_frame(f, 300).unwrap();
    }
    assert!(recv.buffered() >= 600 && recv.ready.is_empty());
    recv.on_frame(ordered(0, None, 300), 300).unwrap();
    assert_eq!((recv.buffered(), recv.ready.len()), (0, 3));
}
