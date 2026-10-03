//! xxHash64, the id of chunk blobs (`LevelChunk.blobs`, `SubChunkEntryItem.blob_id`, `Blob.hash`).

const P1: u64 = 0x9e37_79b1_85eb_ca87;
const P2: u64 = 0xc2b2_ae3d_27d4_eb4f;
const P3: u64 = 0x1656_67b1_9e37_79f9;
const P4: u64 = 0x85eb_ca77_c2b2_ae63;
const P5: u64 = 0x27d4_eb2f_1656_67c5;

/// xxHash64 with seed 0: how the server names blobs.
pub fn xxh64(data: &[u8]) -> u64 {
    let round = |acc: u64, lane: u64| acc.wrapping_add(lane.wrapping_mul(P2)).rotate_left(31).wrapping_mul(P1);
    let u64_at = |b: &[u8]| u64::from_le_bytes(b[..8].try_into().expect("8 bytes"));
    let mut rest = data;
    let mut h = if data.len() >= 32 {
        let mut v = [P1.wrapping_add(P2), P2, 0, 0u64.wrapping_sub(P1)];
        while rest.len() >= 32 {
            for (i, lane) in v.iter_mut().enumerate() {
                *lane = round(*lane, u64_at(&rest[i * 8..]));
            }
            rest = &rest[32..];
        }
        let mut h = v[0].rotate_left(1).wrapping_add(v[1].rotate_left(7)).wrapping_add(v[2].rotate_left(12)).wrapping_add(v[3].rotate_left(18));
        for lane in v {
            h = (h ^ round(0, lane)).wrapping_mul(P1).wrapping_add(P4);
        }
        h
    } else {
        P5
    };
    h = h.wrapping_add(data.len() as u64);
    while rest.len() >= 8 {
        h = (h ^ round(0, u64_at(rest))).rotate_left(27).wrapping_mul(P1).wrapping_add(P4);
        rest = &rest[8..];
    }
    if rest.len() >= 4 {
        let lane = u64::from(u32::from_le_bytes(rest[..4].try_into().expect("4 bytes")));
        h = (h ^ lane.wrapping_mul(P1)).rotate_left(23).wrapping_mul(P2).wrapping_add(P3);
        rest = &rest[4..];
    }
    for &b in rest {
        h = (h ^ u64::from(b).wrapping_mul(P5)).rotate_left(11).wrapping_mul(P1);
    }
    h ^= h >> 33;
    h = h.wrapping_mul(P2);
    h ^= h >> 29;
    h = h.wrapping_mul(P3);
    h ^ (h >> 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_vectors() {
        assert_eq!(xxh64(b""), 0xef46_db37_51d8_e999);
        assert_eq!(xxh64(b"abc"), 0x44bc_2cf5_ad77_0999);
        assert_eq!(xxh64(b"Nobody inspects the spammish repetition"), 0xfbce_a83c_8a37_8bf1);
    }
}
