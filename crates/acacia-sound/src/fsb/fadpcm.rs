//! FMOD FADPCM, ported from vgmstream `src/coding/fadpcm_decoder.c` (byte-accurate to FMOD's DLLs
//! per vgmstream). XA-like ADPCM in fixed 0x8c-byte mono frames, interleaved frame by frame across channels.

/// Bytes per frame (one channel).
pub const FRAME_BYTES: usize = 0x8c;
/// Samples per frame: 0x80 bytes of nibbles after a 0xc header.
pub const FRAME_SAMPLES: usize = (FRAME_BYTES - 0xc) * 2;

/// XA/PSX coefs << 6, as FMOD tweaks them; indices 5..7 are zero.
const COEFS: [[i32; 2]; 8] = [[0, 0], [60, 0], [122, 60], [115, 52], [98, 55], [0, 0], [0, 0], [0, 0]];

/// Decodes `num_samples` interleaved frames of `channels` channels from `data`. Missing data decodes
/// as silence, as vgmstream's zero-filled frame buffer does.
pub fn decode(data: &[u8], channels: usize, num_samples: usize) -> Vec<i16> {
    let mut out = vec![0i16; num_samples * channels];
    let mut frame = [0i16; FRAME_SAMPLES];
    for (block, chunk) in out.chunks_mut(FRAME_SAMPLES * channels).enumerate() {
        for ch in 0..channels {
            let at = (block * channels + ch) * FRAME_BYTES;
            decode_frame(data.get(at..).unwrap_or(&[]), &mut frame);
            for (dst, &s) in chunk.iter_mut().skip(ch).step_by(channels).zip(&frame) {
                *dst = s;
            }
        }
    }
    out
}

/// Decodes one frame; `bytes` past the frame are ignored and a short frame is zero-padded.
fn decode_frame(bytes: &[u8], out: &mut [i16; FRAME_SAMPLES]) {
    let mut f = [0u8; FRAME_BYTES];
    let n = bytes.len().min(FRAME_BYTES);
    f[..n].copy_from_slice(&bytes[..n]);
    let u32_at = |at: usize| u32::from_le_bytes(f[at..at + 4].try_into().unwrap());
    let coefs = u32_at(0);
    let shifts = u32_at(4);
    let mut hist1 = i32::from(i16::from_le_bytes([f[8], f[9]]));
    let mut hist2 = i32::from(i16::from_le_bytes([f[10], f[11]]));

    let mut i = 0;
    for set in 0..8 {
        // FMOD reduces the 4-bit coef index mod 7, so 7 aliases 0 (and >7 would repeat).
        let [c1, c2] = COEFS[((coefs >> (set * 4)) & 0xf) as usize % 7];
        let shift = 22 - ((shifts >> (set * 4)) & 0xf);
        for word in 0..4 {
            let nibbles = u32_at(0xc + 0x10 * set as usize + 4 * word);
            for k in 0..8 {
                let n = ((nibbles >> (k * 4)) & 0xf) << 28;
                let s = (((n as i32) >> shift) - hist2 * c2 + hist1 * c1) >> 6;
                let s = s.clamp(i16::MIN.into(), i16::MAX.into());
                out[i] = s as i16;
                i += 1;
                hist2 = hist1;
                hist1 = s;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_frame_is_silent() {
        assert_eq!(decode(&[0; FRAME_BYTES], 1, FRAME_SAMPLES), vec![0; FRAME_SAMPLES]);
    }

    #[test]
    fn history_and_shift() {
        let mut f = [0u8; FRAME_BYTES];
        f[0] = 0x01; // set 0 uses coefs [60, 0]
        f[4] = 0x0c; // set 0 shift 12 -> nibble << 28 >> 10
        f[8..10].copy_from_slice(&1000i16.to_le_bytes());
        f[0xc] = 0x01; // first nibble +1, the rest 0
        let out = decode(&f, 1, 3);
        let s0 = ((1 << 18) + 1000 * 60) >> 6;
        let s1 = (s0 * 60) >> 6;
        let s2 = (s1 * 60) >> 6;
        assert_eq!(out, vec![s0 as i16, s1 as i16, s2 as i16]);
    }

    #[test]
    fn negative_nibble_clamps() {
        let mut f = [0u8; FRAME_BYTES];
        f[4] = 0x0f; // shift 15 -> nibble << 28 >> 7
        f[0xc] = 0x08; // -8 << 28 >> 7 >> 6 = -262144
        assert_eq!(decode(&f, 1, 1), vec![i16::MIN]);
    }

    #[test]
    fn stereo_interleaves_frames() {
        let mut data = vec![0u8; FRAME_BYTES * 2];
        data[8..10].copy_from_slice(&64i16.to_le_bytes());
        data[0] = 0x01;
        data[FRAME_BYTES + 8..FRAME_BYTES + 10].copy_from_slice(&(-64i16).to_le_bytes());
        data[FRAME_BYTES] = 0x01;
        let out = decode(&data, 2, 2);
        assert_eq!(out, vec![60, -60, 56, -57]);
    }
}
