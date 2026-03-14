use silero_vad_rust::silero_vad::model::load_silero_vad;
use silero_vad_rust::silero_vad::utils_vad::{VadEvent, VadIterator, VadIteratorParams};
use std::sync::Arc;

const VAD_CHUNK_SIZE: usize = 512;

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
    /// Speech ended, and here's the transcription.
    SpeechEnded { segment: TranscribedSegment },
    /// No state change (still silent or still speaking).
    None,
    /// VAD or whisper error (non-fatal, processing continues).
    Error(String),
}

/// Streaming speech transcriber: Silero VAD for speech boundaries,
/// whisper-cpp for transcription of completed segments.
pub struct SpeechTranscriber {
    whisper_state: whisper_cpp_plus::WhisperState,
    vad_iter: VadIterator,

    // Buffering for 512-sample VAD chunks
    sample_buf: Vec<f32>,
    // Audio for the current speech segment (between Start and End)
    speech_audio: Vec<f32>,
    // Rolling buffer of recent audio (for backfilling speech start)
    all_audio: Vec<f32>,
    // Whether we're currently in a speech segment
    speech_start_sample: Option<usize>,
    // Total samples processed through the VAD
    total_samples: usize,
}

impl SpeechTranscriber {
    /// Create a new transcriber with a shared WhisperContext.
    /// Only loads the Silero VAD model (bundled); the whisper context
    /// is provided pre-loaded.
    pub fn new(
        whisper_ctx: Arc<whisper_cpp_plus::WhisperContext>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let silero_model = load_silero_vad()?;
        let vad_params = VadIteratorParams {
            threshold: 0.5,
            sampling_rate: 16_000,
            min_silence_duration_ms: 300,
            speech_pad_ms: 30,
        };
        let vad_iter = VadIterator::new(silero_model, vad_params)?;
        let whisper_state = whisper_ctx.create_state()?;

        Ok(Self {
            whisper_state,
            vad_iter,
            sample_buf: Vec::new(),
            speech_audio: Vec::new(),
            all_audio: Vec::new(),
            speech_start_sample: None,
            total_samples: 0,
        })
    }

    /// Feed 16kHz mono f32 audio samples. Returns speech events for each
    /// VAD chunk boundary crossed. Call this as audio arrives.
    pub fn feed(&mut self, samples: &[f32]) -> Vec<SpeechEvent> {
        self.sample_buf.extend_from_slice(samples);
        self.all_audio.extend_from_slice(samples);

        let mut events = Vec::new();

        while self.sample_buf.len() >= VAD_CHUNK_SIZE {
            let chunk: Vec<f32> = self.sample_buf.drain(..VAD_CHUNK_SIZE).collect();
            self.total_samples += VAD_CHUNK_SIZE;

            if self.speech_start_sample.is_some() {
                self.speech_audio.extend_from_slice(&chunk);
            }

            match self.vad_iter.process_chunk(&chunk, false, 0) {
                Ok(Some(VadEvent::Start(pos))) => {
                    let start_sample = pos as usize;
                    self.speech_start_sample = Some(start_sample);

                    // Backfill: grab audio from the start position (which may be
                    // padded back by speech_pad_ms before the current chunk).
                    let offset_in_all = if start_sample < self.total_samples {
                        self.all_audio
                            .len()
                            .saturating_sub(self.total_samples - start_sample)
                    } else {
                        self.all_audio.len()
                    };
                    self.speech_audio = self.all_audio[offset_in_all..].to_vec();

                    events.push(SpeechEvent::SpeechStarted {
                        sample: start_sample,
                    });
                }
                Ok(Some(VadEvent::End(pos))) => {
                    let end_sample = pos as usize;

                    let segment = if !self.speech_audio.is_empty() {
                        transcribe_audio(&self.whisper_ctx, &self.speech_audio).map(|text| {
                            TranscribedSegment {
                                text,
                                start_sample: self.speech_start_sample.unwrap_or(0),
                                end_sample,
                            }
                        })
                    } else {
                        None
                    };

                    self.speech_start_sample = None;
                    self.speech_audio.clear();
                    self.all_audio.clear();

                    if let Some(segment) = segment {
                        events.push(SpeechEvent::SpeechEnded { segment });
                    }
                }
                Ok(None) => {
                    events.push(SpeechEvent::None);
                }
                Err(e) => {
                    events.push(SpeechEvent::Error(e.to_string()));
                }
            }
        }

        events
    }

    /// Flush: if speech is in progress, transcribe what we have so far.
    /// Call this when the audio stream ends.
    pub fn flush(&mut self) -> Option<TranscribedSegment> {
        if self.speech_start_sample.is_none() || self.speech_audio.is_empty() {
            return None;
        }

        let result = transcribe_audio(&self.whisper_ctx, &self.speech_audio).map(|text| {
            TranscribedSegment {
                text,
                start_sample: self.speech_start_sample.unwrap_or(0),
                end_sample: self.total_samples,
            }
        });

        self.speech_start_sample = None;
        self.speech_audio.clear();
        self.all_audio.clear();

        result
    }

    /// Whether we're currently inside a speech segment.
    pub fn is_speaking(&self) -> bool {
        self.speech_start_sample.is_some()
    }

    /// Duration of the current speech segment so far (seconds).
    pub fn speech_duration_secs(&self) -> f64 {
        self.speech_audio.len() as f64 / 16000.0
    }

    /// Total samples processed.
    pub fn total_samples(&self) -> usize {
        self.total_samples
    }
}

fn transcribe_audio(
    whisper_ctx: &whisper_cpp_plus::WhisperContext,
    audio: &[f32],
) -> Option<String> {
    let params =
        whisper_cpp_plus::FullParams::new(whisper_cpp_plus::SamplingStrategy::BeamSearch {
            beam_size: 5,
        })
        .language("en")
        .no_context(true);

    let mut state = match whisper_ctx.create_state() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("whisper state error: {e}");
            return None;
        }
    };

    if let Err(e) = state.full(params, audio) {
        tracing::warn!("whisper error: {e}");
        return None;
    }

    let n_segments = state.full_n_segments();
    let mut text = String::new();
    for i in 0..n_segments {
        if let Ok(t) = state.full_get_segment_text(i) {
            let t = t.trim();
            if !t.is_empty() {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(t);
            }
        }
    }

    if text.is_empty() { None } else { Some(text) }
}
