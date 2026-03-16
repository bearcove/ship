use std::sync::Arc;

use super::{AudioHost, AudioStream};

pub struct NoopAudioHost;

struct NoopStream;

impl AudioStream for NoopStream {
    fn stop(self: Box<Self>) {}
}

impl AudioHost for NoopAudioHost {
    fn play_samples(
        &self,
        _samples: Arc<[f32]>,
        _sample_rate: u32,
    ) -> anyhow::Result<(Box<dyn AudioStream>, tokio::sync::oneshot::Receiver<()>)> {
        anyhow::bail!("audio playback not available (built without `audio` feature)")
    }

    fn capture_mic(
        &self,
    ) -> anyhow::Result<(
        Box<dyn AudioStream>,
        tokio::sync::mpsc::UnboundedReceiver<Vec<f32>>,
    )> {
        anyhow::bail!("mic capture not available (built without `audio` feature)")
    }
}
