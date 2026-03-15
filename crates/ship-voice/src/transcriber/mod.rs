/// A completed speech segment with its transcribed text.
pub struct TranscribedSegment {
    pub text: String,
    pub start_sample: usize,
    pub end_sample: usize,
}

/// Events emitted by the speech transcriber as audio flows through.
pub enum SpeechEvent {
    /// Speech started at this sample position.
    SpeechStarted { sample: usize },
    /// Speech ended, and here is the transcription.
    SpeechEnded { segment: TranscribedSegment },
    /// No state change (still silent or still speaking).
    None,
    /// VAD or transcription error (non-fatal, processing continues).
    Error(String),
}

/// Streaming speech transcriber: detects speech boundaries and transcribes
/// completed segments.
pub trait SpeechTranscriber {
    /// Feed 16 kHz mono f32 audio samples. Returns speech events for each
    /// VAD chunk boundary crossed.
    fn feed(&mut self, samples: &[f32]) -> Vec<SpeechEvent>;

    /// Flush: if speech is in progress, transcribe what we have so far.
    /// Call this when the audio stream ends.
    fn flush(&mut self) -> Option<TranscribedSegment>;

    /// Whether we are currently inside a speech segment.
    fn is_speaking(&self) -> bool;

    /// Duration of the current speech segment so far (seconds).
    fn speech_duration_secs(&self) -> f64;

    /// Total samples processed.
    fn total_samples(&self) -> usize;
}

/// Factory for creating [`SpeechTranscriber`] instances.
///
/// The factory holds shared state (e.g. a loaded model) and can create
/// multiple independent transcriber sessions from it.
pub trait TranscriberFactory: Send + Sync {
    /// Create a new transcriber session.
    fn create(&self) -> Result<Box<dyn SpeechTranscriber + Send>, String>;
}

#[cfg(feature = "whisper")]
mod whisper_backend;

/// Try to load a transcriber factory.
///
/// When the `whisper` feature is enabled, this resolves a whisper model from:
/// 1. `explicit_path` if provided
/// 2. `SHIP_WHISPER_MODEL` env var
/// 3. Platform data dir / home dir defaults
///
/// Returns `None` when the feature is disabled or no model is found.
pub fn load_transcriber_factory(
    explicit_path: Option<&str>,
) -> Option<Box<dyn TranscriberFactory>> {
    #[cfg(feature = "whisper")]
    {
        whisper_backend::load_factory(explicit_path)
    }

    #[cfg(not(feature = "whisper"))]
    {
        let _ = explicit_path;
        tracing::info!("voice transcription not available (whisper feature not enabled)");
        None
    }
}
