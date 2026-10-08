//! Playing decoded samples at a place: volume falls linearly to silence at the sound's range (Java:
//! 16 blocks × the volume when above 1; Bedrock: the definition's `max_distance`), and the left and
//! right channels follow where the sound is relative to the listener's facing.

use std::num::NonZero;
use std::sync::Arc;

use rodio::buffer::SamplesBuffer;
use rodio::source::ChannelVolume;
use rodio::{MixerDeviceSink, Source};

use crate::fsb::Pcm;

/// Where the player's ears are.
#[derive(Debug, Clone, Copy)]
pub struct Listener {
    pub position: [f64; 3],
    /// Unit vector to the listener's right, horizontal.
    pub right: [f32; 3],
}

/// One sound to start: the sample, how loud and fast, and where (`None`: everywhere, as UI sounds).
pub struct Voice {
    pub pcm: Arc<Pcm>,
    pub volume: f32,
    pub pitch: f32,
    pub position: Option<[f64; 3]>,
    /// Blocks where it falls silent.
    pub range: f32,
}

/// The audio device; without one (no device, a headless test) playing does nothing.
pub struct Output {
    sink: Option<MixerDeviceSink>,
}

impl Output {
    pub fn open() -> Output {
        let sink = rodio::DeviceSinkBuilder::open_default_sink().inspect_err(|e| tracing::warn!(%e, "no audio device")).ok();
        Output { sink }
    }

    pub fn play(&self, voice: Voice, listener: &Listener) {
        let Some(sink) = &self.sink else { return };
        let (gain, [left, right]) = place(&voice, listener);
        if gain <= 0.0 || voice.pcm.samples.is_empty() {
            return;
        }
        let (Some(channels), Some(rate)) = (NonZero::new(voice.pcm.channels.max(1)), NonZero::new(voice.pcm.sample_rate.max(1))) else { return };
        let samples: Vec<f32> = voice.pcm.samples.iter().map(|&s| f32::from(s) / 32768.0).collect();
        let source = SamplesBuffer::new(channels, rate, samples).speed(voice.pitch.clamp(0.5, 2.0)).amplify(gain);
        sink.mixer().add(ChannelVolume::new(source, vec![left, right]));
    }
}

/// Overall gain and the two channels' share.
fn place(voice: &Voice, listener: &Listener) -> (f32, [f32; 2]) {
    let Some(at) = voice.position else { return (voice.volume.min(1.0), [1.0, 1.0]) };
    let d = [at[0] - listener.position[0], at[1] - listener.position[1], at[2] - listener.position[2]];
    let distance = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() as f32;
    let falloff = (1.0 - distance / voice.range.max(1.0)).clamp(0.0, 1.0);
    let side = if distance > 1e-3 { (d[0] as f32 * listener.right[0] + d[2] as f32 * listener.right[2]) / distance } else { 0.0 };
    // A sound straight ahead plays at full in both ears; to one side, the far ear keeps 30%.
    let ear = |toward: f32| 1.0 - 0.7 * (-toward).max(0.0);
    (voice.volume.min(1.0) * falloff, [ear(-side), ear(side)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(at: [f64; 3]) -> Voice {
        Voice { pcm: Arc::new(Pcm { sample_rate: 44100, channels: 1, samples: vec![0] }), volume: 1.0, pitch: 1.0, position: Some(at), range: 16.0 }
    }

    #[test]
    fn far_sounds_fade_and_side_sounds_pan() {
        let listener = Listener { position: [0.0; 3], right: [-1.0, 0.0, 0.0] };
        let (near, _) = place(&voice([0.0, 0.0, 4.0]), &listener);
        let (far, _) = place(&voice([0.0, 0.0, 12.0]), &listener);
        assert!((near - 0.75).abs() < 1e-5 && (far - 0.25).abs() < 1e-5, "{near} {far}");
        assert_eq!(place(&voice([0.0, 0.0, 20.0]), &listener).0, 0.0);
        let (_, [left, right]) = place(&voice([-4.0, 0.0, 0.0]), &listener);
        assert!(right > left, "a sound on the right is louder on the right: {left} {right}");
    }
}
