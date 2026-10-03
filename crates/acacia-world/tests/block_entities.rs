mod common;

use acacia_proto::nbt::{self, Nbt, Network, Value};
use acacia_world::{Dimension, Error, level_chunk_block_entities, sub_chunk_block_entities};
use bytes::BytesMut;
use common::*;

const OW: Dimension = Dimension::overworld(0);

fn sign(x: i32) -> Nbt {
    let value = Value::Compound(vec![
        ("id".into(), Value::String("Sign".into())),
        ("x".into(), Value::Int(x)),
        ("y".into(), Value::Int(64)),
        ("z".into(), Value::Int(-3)),
    ]);
    Nbt { name: String::new(), value }
}

fn block_entities() -> Vec<u8> {
    let mut w = BytesMut::new();
    nbt::write::<Network>(&mut w, &sign(1));
    nbt::write::<Network>(&mut w, &sign(2));
    w.to_vec()
}

/// Two sections, 24 biome storages (single, copies, one packed), two border blocks.
fn full_chunk_head() -> Vec<u8> {
    let mut out = section_v9(-4, &[&filled(1)]);
    let mixed: Vec<u32> = (0..VOLUME as u32).map(|i| i % 3).collect();
    out.extend(section_v9(0, &[&mixed, &filled(0)]));
    storage(&mut out, &filled(7), None);
    out.extend([0xff; 10]);
    storage(&mut out, &mixed, None);
    out.extend([0xff; 12]);
    out.extend([2, 0x11, 0x22]);
    out
}

#[test]
fn level_chunk_block_entities_follow_sections_biomes_and_border() {
    let head = full_chunk_head();
    let mut payload = head.clone();
    payload.extend(block_entities());
    assert_eq!(level_chunk_block_entities(&payload, 2, Some(OW)), Ok(head.len()));
    let mut tail = &payload[head.len()..];
    assert_eq!(nbt::read::<Network>(&mut tail).unwrap(), sign(1));
    assert_eq!(nbt::read::<Network>(&mut tail).unwrap(), sign(2));
    assert!(tail.is_empty());
    assert_eq!(level_chunk_block_entities(&head, 2, Some(OW)), Ok(head.len()), "no block entities");
}

#[test]
fn cache_and_request_mode_payloads() {
    let mut cached = vec![0];
    cached.extend(block_entities());
    assert_eq!(level_chunk_block_entities(&cached, 0, None), Ok(1), "cache mode: border, then block entities");

    let mut biomes_only = Vec::new();
    storage(&mut biomes_only, &filled(7), None);
    biomes_only.extend([0xff; 23]);
    assert_eq!(level_chunk_block_entities(&biomes_only, 0, Some(OW)), Ok(biomes_only.len()), "no border byte");
}

#[test]
fn sub_chunk_entry_block_entities_follow_the_section() {
    let section = section_v9(3, &[&filled(5)]);
    let mut payload = section.clone();
    payload.extend(block_entities());
    assert_eq!(sub_chunk_block_entities(&payload), Ok(section.len()));
}

#[test]
fn truncated_payloads_error() {
    let head = full_chunk_head();
    assert_eq!(level_chunk_block_entities(&head[..head.len() - 30], 2, Some(OW)), Err(Error::UnexpectedEof));
    assert_eq!(sub_chunk_block_entities(&[7]), Err(Error::SectionVersion(7)));
}
