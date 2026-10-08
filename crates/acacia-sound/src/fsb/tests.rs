use super::*;

/// A version-1 FSB5 bank with one sample: `mode` is the sample header's 64-bit mode word sans the
/// extra-flags bit, `extra` the raw extra-flag chunks, `names` the raw name table.
fn bank(codec: u32, flags: u32, mode: u64, extra: &[u8], names: &[u8], data: &[u8]) -> Vec<u8> {
    let mut headers = (mode | u64::from(!extra.is_empty())).to_le_bytes().to_vec();
    headers.extend_from_slice(extra);
    let mut b = b"FSB5".to_vec();
    for v in [1, 1, headers.len() as u32, names.len() as u32, data.len() as u32, codec, 0, flags] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.resize(0x3c, 0);
    b.extend_from_slice(&headers);
    b.extend_from_slice(names);
    b.extend_from_slice(data);
    b
}

fn mode(num_samples: u64, channels_code: u64, rate_code: u64) -> u64 {
    num_samples << 34 | channels_code << 5 | rate_code << 1
}

fn extra(kind: u32, body: &[u8], more: bool) -> Vec<u8> {
    let flag = kind << 25 | (body.len() as u32) << 1 | u32::from(more);
    [&flag.to_le_bytes()[..], body].concat()
}

#[test]
fn pcm16_mono() {
    let data: Vec<u8> = [1i16, -2, 300].iter().flat_map(|s| s.to_le_bytes()).collect();
    let pcm = decode(&bank(CODEC_PCM16, 0, mode(3, 0, 8), &[], &[], &data)).unwrap();
    assert_eq!(pcm, Pcm { sample_rate: 44100, channels: 1, samples: vec![1, -2, 300] });
}

#[test]
fn pcm16_big_endian_stereo_pads_short_data() {
    let data: Vec<u8> = [1i16, -2, 300].iter().flat_map(|s| s.to_be_bytes()).collect();
    let pcm = decode(&bank(CODEC_PCM16, 1, mode(2, 1, 9), &[], &[], &data)).unwrap();
    assert_eq!((pcm.sample_rate, pcm.channels), (48000, 2));
    assert_eq!(pcm.samples, vec![1, -2, 300, 0]);
}

#[test]
fn extra_flags_override_rate_and_channels_and_give_loop() {
    let chunks = [
        extra(2, &37035u32.to_le_bytes(), true),
        extra(1, &[3, 0, 0, 0], true),
        extra(3, &[5u32.to_le_bytes(), 9u32.to_le_bytes()].concat(), false),
    ]
    .concat();
    let bytes = bank(CODEC_PCM16, 0, mode(10, 0, 0), &chunks, &[], &[0; 60]);
    let s = &parse(&bytes).unwrap()[0];
    assert_eq!((s.sample_rate, s.channels, s.num_samples, s.looping), (37035, 3, 10, Some((5, 10))));
    assert_eq!(decode(&bytes).unwrap().samples.len(), 30);
}

#[test]
fn name_table() {
    let names = [&4u32.to_le_bytes()[..], b"pop\0"].concat();
    let bytes = bank(CODEC_FADPCM, 0, mode(1, 0, 9), &[], &names, &[0; 0x8c]);
    assert_eq!(parse(&bytes).unwrap()[0].name, Some("pop"));
    assert_eq!(decode(&bytes).unwrap().samples, vec![0]);
}

#[test]
fn errors() {
    assert_eq!(decode(b"RIFF0000"), Err(Error::NotFsb5));
    assert_eq!(decode(b"FSB5"), Err(Error::Truncated));
    let vorbis = bank(0x0f, 0, mode(1, 0, 9), &[], &[], &[0; 32]);
    assert_eq!(decode(&vorbis), Err(Error::Unsupported(0x0f)));
    assert_eq!(Error::Unsupported(0x0f).to_string(), "unsupported FSB5 codec VORBIS (0xf)");
    let mut short = bank(CODEC_PCM16, 0, mode(1, 0, 9), &[], &[], &[0; 2]);
    short.pop();
    assert!(matches!(decode(&short), Err(Error::Malformed(_))));
    let mut v2 = bank(CODEC_PCM16, 0, mode(1, 0, 9), &[], &[], &[0; 2]);
    v2[4] = 2;
    assert_eq!(decode(&v2), Err(Error::Version(2)));
}
