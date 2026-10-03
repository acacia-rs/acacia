//! Encoder for network sub-chunks, the inverse of the crate's decoder.
#![allow(dead_code)]

pub const VOLUME: usize = 4096;

pub fn idx(x: usize, y: usize, z: usize) -> usize {
    (x << 8) | (z << 4) | y
}

fn var_u32(out: &mut Vec<u8>, mut v: u32) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn var_i32(out: &mut Vec<u8>, v: i32) {
    var_u32(out, ((v << 1) ^ (v >> 31)) as u32);
}

/// Encodes one storage; `force_bits` overrides the smallest fitting size.
pub fn storage(out: &mut Vec<u8>, values: &[u32], force_bits: Option<u8>) {
    let mut palette: Vec<u32> = Vec::new();
    let indices: Vec<usize> = values
        .iter()
        .map(|v| palette.iter().position(|p| p == v).unwrap_or_else(|| {
            palette.push(*v);
            palette.len() - 1
        }))
        .collect();
    let needed = [0u8, 1, 2, 3, 4, 5, 6, 8, 16]
        .into_iter()
        .find(|&b| (1usize << b) >= palette.len())
        .unwrap();
    let bits = force_bits.unwrap_or(needed);
    out.push((bits << 1) | 1);
    if bits == 0 {
        var_i32(out, palette[0] as i32);
        return;
    }
    let per = 32 / bits as usize;
    let mut words = vec![0u32; VOLUME.div_ceil(per)];
    for (i, &p) in indices.iter().enumerate() {
        words[i / per] |= (p as u32) << ((i % per) * bits as usize);
    }
    words.iter().for_each(|w| out.extend_from_slice(&w.to_le_bytes()));
    var_i32(out, palette.len() as i32);
    palette.iter().for_each(|&p| var_i32(out, p as i32));
}

/// Version-9 section with the given layers.
pub fn section_v9(y_index: i8, layers: &[&[u32]]) -> Vec<u8> {
    let mut out = vec![9, layers.len() as u8, y_index as u8];
    layers.iter().for_each(|l| storage(&mut out, l, None));
    out
}

pub fn filled(id: u32) -> Vec<u32> {
    vec![id; VOLUME]
}
