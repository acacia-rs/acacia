//! Hostile and edge-case input (README.md, "Limits").

use acacia_nbt::{Error, Flavor, LittleEndian, List, MAX_DEPTH, Nbt, Network, Raw, Value, read, read_lossy, skip, write};
use bytes::BytesMut;

fn both<F: Flavor>(data: &[u8]) -> Result<Nbt, Error> {
    let decoded = read::<F>(&mut &data[..]);
    assert_eq!(skip::<F>(&mut &data[..]).err(), decoded.as_ref().err().cloned());
    decoded
}

/// A little-endian root list nested `levels` deep around an empty byte list.
fn nested_lists(levels: usize) -> Vec<u8> {
    let mut doc = vec![9, 0, 0];
    for _ in 0..levels {
        doc.extend([9, 1, 0, 0, 0]);
    }
    doc.extend([1, 0, 0, 0, 0]);
    doc
}

#[test]
fn list_of_end_tags_with_a_length_is_rejected() {
    assert_eq!(both::<LittleEndian>(&[9, 0, 0, 0, 0xff, 0xff, 0xff, 0x7f]), Err(Error::EndList(i32::MAX as usize)));
    assert_eq!(both::<Network>(&[9, 0, 0, 2]), Err(Error::EndList(1)));
    let empty = both::<Network>(&[9, 0, 0, 0]).unwrap();
    assert_eq!(empty.value, Value::List(List { tag: 0, items: vec![] }));
}

#[test]
fn counts_beyond_the_input_end_as_eof() {
    let huge_list = [9, 0, 0, 1, 0xff, 0xff, 0xff, 0x7f, 5];
    assert!(matches!(both::<LittleEndian>(&huge_list), Err(Error::Eof { .. })));
    let huge_long_array = [12, 0, 0, 0xff, 0xff, 0xff, 0x7f];
    assert!(matches!(both::<LittleEndian>(&huge_long_array), Err(Error::Eof { .. })));
}

#[test]
fn negative_length_is_rejected() {
    assert_eq!(both::<LittleEndian>(&[7, 0, 0, 0xff, 0xff, 0xff, 0xff]), Err(Error::InvalidLength(-1)));
    assert_eq!(both::<Network>(&[7, 0, 1]), Err(Error::InvalidLength(-1)));
}

#[test]
fn unknown_tag_is_rejected() {
    assert_eq!(both::<Network>(&[13, 0]), Err(Error::UnknownTag(13)));
    assert_eq!(both::<Network>(&[10, 0, 99, 0]), Err(Error::UnknownTag(99)));
}

#[test]
fn nesting_stops_at_max_depth() {
    assert!(both::<LittleEndian>(&nested_lists(MAX_DEPTH)).is_ok());
    assert_eq!(both::<LittleEndian>(&nested_lists(MAX_DEPTH + 1)), Err(Error::TooDeep(MAX_DEPTH)));
}

#[test]
fn invalid_utf8_is_replaced_and_reported() {
    let doc = [8, 0, 0, 2, 0, b'a', 0xff];
    let (nbt, lossy) = read_lossy::<LittleEndian>(&mut &doc[..]).unwrap();
    assert_eq!(nbt.value, Value::String("a\u{fffd}".into()));
    assert!(lossy);
    assert!(!read_lossy::<LittleEndian>(&mut &[8, 0, 0, 1, 0, b'a'][..]).unwrap().1);
}

#[test]
fn string_too_long_for_its_prefix_is_cut_at_a_character() {
    let nbt = Nbt { name: String::new(), value: Value::String("é".repeat(40_000).into()) };
    let mut w = BytesMut::new();
    write::<LittleEndian>(&mut w, &nbt);
    let mut r = &w[..];
    let back = read::<LittleEndian>(&mut r).unwrap();
    assert!(r.is_empty());
    assert_eq!(back.value, Value::String("é".repeat(32_767).into()));
}

/// Every list item and compound entry pays for this; a new variant must not grow it.
#[test]
fn value_is_four_words() {
    assert_eq!(size_of::<Value>(), 32);
    assert_eq!(size_of::<acacia_nbt::Str>(), 24);
}

#[test]
fn raw_keeps_the_bytes_and_decodes_on_demand() {
    let doc = [10, 0, 1, 1, b'a', 7, 0, 99];
    let mut r = &doc[..];
    let raw = Raw::<Network>::read(&mut r).unwrap();
    assert_eq!((raw.as_bytes(), r), (&doc[..7], &[99][..]));
    assert_eq!(raw.decode().unwrap().value, Value::Compound(vec![("a".into(), Value::Byte(7))]));
    let mut w = BytesMut::new();
    raw.write(&mut w);
    assert_eq!(&w[..], &doc[..7]);
    assert!(Raw::<Network>::default().is_end() && !raw.is_end());
    assert_eq!(Raw::<Network>::from(&raw.decode().unwrap()), raw);
}

#[test]
fn raw_reports_invalid_utf8_without_decoding() {
    let (_, lossy) = Raw::<LittleEndian>::read_lossy(&mut &[8, 0, 0, 2, 0, b'a', 0xff][..]).unwrap();
    assert!(lossy);
}

#[test]
fn end_root_is_one_byte() {
    let mut w = BytesMut::new();
    write::<Network>(&mut w, &Nbt::default());
    assert_eq!(&w[..], [0]);
    let mut r = &[0, 7][..];
    assert_eq!(read::<Network>(&mut r).unwrap(), Nbt::default());
    assert_eq!(r, [7]);
}
