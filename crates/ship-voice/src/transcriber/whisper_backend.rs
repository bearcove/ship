use std::path::PathBuf;
use std::sync::Arc;

use silero_vad_rust::silero_vad::model::load_silero_vad;
use silero_vad_rust::silero_vad::utils_vad::{VadEvent, VadIterator, VadIteratorParams};

use super::{SpeechEvent, SpeechTranscriber, TranscriberFactory, TranscribedSegment};

const DEFAULT_MODEL_FILENAME: &str = "ggml-base.en.bin";
const VAD_CHUNK_SIZE: usize = 512;

struct WhisperFactory {
    ctx: Arc<whisper_cpp_plus::WhisperContext>,
}

struct WhisperTranscriber {
    whisper_state: whisper_cpp_plus::WhisperState,
    vad_iter: VadIterator,
    sample_buf: Vec<f32>,
    speech_audio: Vec<f32>,
    all_audio: Vec<f32>,
    speech_start_sample: Option<usize>,
    total_samples: usize,
}

/// Try to load a whisper-backed transcriber factory. Returns `None` if no model
/// is found.
pub(super) fn load_factory(
    explicit_path: Option<&str>,
) -> Option<Box<dyn TranscriberFactory>> {
    let path = resolve_model_path(explicit_path)?;

    tracing::info!(path = %path.display(), "loading whisper model");
    let ctx = match whisper_cpp_plus::WhisperContext::new(path.to_str().unwrap()) {
        Ok(ctx) => {
            tracing::info!(path = %path.display(), "whisper model loaded");
            Arc::new(ctx)
        }
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "failed to load whisper model");
            return None;
        }
    };

    Some(Box::new(WhisperFactory { ctx }))
}

impl TranscriberFactory for WhisperFactory {
    fn create(&self) -> Result<Box<dyn SpeechTranscriber + Send>, String> {
        let silero_model = load_silero_vad().map_err(|e| format!("failed to load VAD: {e}"))?;

        let vad_params = VadIteratorParams {
            threshold: 0.5,
            sampling_rate: 16_000,
            min_silence_duration_ms: 300,
            speech_pad_ms: 30,
        };
        let vad_iter = VadIterator::new(silero_model, vad_params)
            .map_err(|e| format!("failed to create VAD: {e}"))?;

        let whisper_state = self
            .ctx
            .create_state()
            .map_err(|e| format!("failed to create whisper state: {e}"))?;

        Ok(Box::new(WhisperTranscriber {
            whisper_state,
            vad_iter,
            sample_buf: Vec::new(),
            speech_audio: Vec::new(),
            all_audio: Vec::new(),
            speech_start_sample: None,
            total_samples: 0,
        }))
    }
}

fn resolve_model_path(explicit_path: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = explicit_path {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
        tracing::warn!(path = %p.display(), "explicit model path does not exist");
        return None;
    }

    if let Ok(env_path) = std::env::var("SHIP_WHISPER_MODEL") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Some(p);
        }
        tracing::warn!(path = %p.display(), "SHIP_WHISPER_MODEL path does not exist");
        return None;
    }

    let candidates = [
        dirs_next::data_dir().map(|d| d.join("whisper").join(DEFAULT_MODEL_FILENAME)),
        dirs_next::home_dir()
            .map(|d| d.join(".local/share/whisper").join(DEFAULT_MODEL_FILENAME)),
        Some(PathBuf::from(DEFAULT_MODEL_FILENAME)),
    ];
    let found = candidates.into_iter().flatten().find(|p| p.exists());

    if found.is_none() {
        tracing::info!(
            "no whisper model found — voice transcription disabled. \
             Set SHIP_WHISPER_MODEL or place {DEFAULT_MODEL_FILENAME} in \
             ~/.local/share/whisper/"
        );
    }

    found
}

impl SpeechTranscriber for WhisperTranscriber {
    fn feed(&mut self, samples: &[f32]) -> Vec<SpeechEvent> {
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
                        transcribe_audio(&mut self.whisper_state, &self.speech_audio).map(|text| {
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

    fn flush(&mut self) -> Option<TranscribedSegment> {
        if self.speech_start_sample.is_none() || self.speech_audio.is_empty() {
            return None;
        }

        let result = transcribe_audio(&mut self.whisper_state, &self.speech_audio).map(|text| {
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

    fn is_speaking(&self) -> bool {
        self.speech_start_sample.is_some()
    }

    fn speech_duration_secs(&self) -> f64 {
        self.speech_audio.len() as f64 / 16000.0
    }

    fn total_samples(&self) -> usize {
        self.total_samples
    }
}

fn transcribe_audio(
    state: &mut whisper_cpp_plus::WhisperState,
    audio: &[f32],
) -> Option<String> {
    let params =
        whisper_cpp_plus::FullParams::new(whisper_cpp_plus::SamplingStrategy::BeamSearch {
            beam_size: 5,
        })
        .language("en")
        .no_context(true);

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
