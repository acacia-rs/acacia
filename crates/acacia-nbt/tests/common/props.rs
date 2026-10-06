//! Properties every input must hold; run by tests/corpus.rs and the fuzz targets.

use acacia_nbt::{Flavor, Nbt, Raw, read, read_lossy, skip, write};
use bytes::BytesMut;

fn encode<F: Flavor>(nbt: &Nbt) -> BytesMut {
    let mut w = BytesMut::new();
    write::<F>(&mut w, nbt);
    w
}

/// Arbitrary bytes: nothing panics; `skip` and `Raw` accept and consume what `read` does and `Raw`
/// reports the same replaced strings; an accepted document holds [`tree`].
pub fn decode<F: Flavor>(data: &[u8]) {
    let (mut a, mut b, mut c) = (data, data, data);
    let decoded = read_lossy::<F>(&mut a);
    let skipped = skip::<F>(&mut b);
    let raw = Raw::<F>::read_lossy(&mut c);
    assert_eq!(decoded.is_ok(), skipped.is_ok(), "read: {:?}, skip: {skipped:?}", decoded.as_ref().err());
    assert_eq!(decoded.is_ok(), raw.is_ok(), "read: {:?}, raw: {:?}", decoded.as_ref().err(), raw.as_ref().err());
    let (Ok((nbt, lossy)), Ok((raw, raw_lossy))) = (decoded, raw) else { return };
    assert_eq!((a.len(), a.len()), (b.len(), c.len()), "read, skip and raw consumed different lengths");
    assert_eq!(lossy, raw_lossy, "raw reports replaced strings differently");
    assert_eq!(raw.as_bytes(), &data[..data.len() - a.len()]);
    assert_eq!(encode::<F>(&raw.decode().expect("raw decodes")), encode::<F>(&nbt));
    tree::<F>(&nbt);
}

/// A well-formed tree: its encoding decodes and skips to the last byte, and re-encodes to the same bytes.
pub fn tree<F: Flavor>(nbt: &Nbt) {
    let first = encode::<F>(nbt);
    let (mut a, mut b) = (&first[..], &first[..]);
    let back = read::<F>(&mut a).expect("own output decodes");
    skip::<F>(&mut b).expect("own output skips");
    assert!(a.is_empty() && b.is_empty(), "own output not consumed");
    assert_eq!(encode::<F>(&back), first, "re-encoding changed the bytes");
    assert_eq!(Raw::<F>::from(nbt).as_bytes(), &first[..]);
}
