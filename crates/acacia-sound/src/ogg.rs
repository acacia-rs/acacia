//! Ogg Vorbis samples (Java's sounds) decoded whole with lewton.

use std::io::Cursor;

use lewton::inside_ogg::OggStreamReader;

use crate::fsb::Pcm;

pub fn decode(bytes: &[u8]) -> Result<Pcm, lewton::VorbisError> {
    let mut reader = OggStreamReader::new(Cursor::new(bytes))?;
    let (sample_rate, channels) = (reader.ident_hdr.audio_sample_rate, u16::from(reader.ident_hdr.audio_channels));
    let mut samples = Vec::new();
    while let Some(packet) = reader.read_dec_packet_itl()? {
        samples.extend(packet);
    }
    Ok(Pcm { sample_rate, channels, samples })
}
