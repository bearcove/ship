use std::sync::Arc;

/// A handle to a running audio stream. The stream stops when this is dropped.
pub trait AudioStream: Send {
    fn stop(self: Box<Self>);
}

/// Provides audio input (mic capture) and output (speaker playback).
pub trait AudioHost: Send + Sync {
    /// Play mono f32 samples at the given sample rate.
    /// Returns a stream handle and a receiver that fires when playback is done.
    fn play_samples(
        &self,
        samples: Arc<[f32]>,
        sample_rate: u32,
    ) -> anyhow::Result<(Box<dyn AudioStream>, tokio::sync::oneshot::Receiver<()>)>;

    /// Start capturing mic input. Returns mono 16kHz f32 chunks on the channel.
    fn capture_mic(
        &self,
    ) -> anyhow::Result<(
        Box<dyn AudioStream>,
        tokio::sync::mpsc::UnboundedReceiver<Vec<f32>>,
    )>;
}

#[cfg(feature = "audio")]
mod cpal_backend;

#[cfg(feature = "audio")]
pub use cpal_backend::CpalAudioHost;

mod noop_backend;
pub use noop_backend::NoopAudioHost;

/// Returns the best available audio host for the current build.
pub fn default_audio_host() -> Box<dyn AudioHost> {
    #[cfg(feature = "audio")]
    {
        Box::new(CpalAudioHost)
    }
    #[cfg(not(feature = "audio"))]
    {
        Box::new(NoopAudioHost)
    }
}
