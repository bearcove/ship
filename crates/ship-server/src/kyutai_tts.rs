use anyhow::{Result, anyhow};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::generation::{LogitsProcessor, Sampling};

const TTS_REPO: &str = "kyutai/tts-1.6b-en_fr";
const VOICES_REPO: &str = "kyutai/tts-voices";
const LM_FILE: &str = "dsm_tts_1e68beda@240.safetensors";
const MIMI_FILE: &str = "tokenizer-e351c8d8-checkpoint125.safetensors";
const TOKENIZER_FILE: &str = "tokenizer_spm_8k_en_fr_audio.model";
const DEFAULT_VOICE_FILE: &str = "unmute-prod-website/default_voice.wav.1e68beda@240.safetensors";

pub struct KyutaiTtsModel {
    lm: moshi::lm::LmModel,
    audio_tokenizer: moshi::mimi::Mimi,
    text_tokenizer: sentencepiece::SentencePieceProcessor,
    tts_config: moshi::tts_streaming::Config,
    ca_src: Tensor,
    device: Device,
}

impl KyutaiTtsModel {
    /// Load the Kyutai TTS model. Sync; call from `tokio::task::spawn_blocking`.
    pub fn load() -> Result<Self> {
        let api = hf_hub::api::sync::Api::new()?;
        let tts_repo = api.model(TTS_REPO.to_string());
        let voices_repo = api.model(VOICES_REPO.to_string());

        tracing::info!("downloading Kyutai TTS model files (cached after first run)");
        let lm_path = tts_repo.get(LM_FILE)?;
        let mimi_path = tts_repo.get(MIMI_FILE)?;
        let tokenizer_path = tts_repo.get(TOKENIZER_FILE)?;
        let voice_path = voices_repo.get(DEFAULT_VOICE_FILE)?;

        let device = Device::new_metal(0).unwrap_or(Device::Cpu);
        tracing::info!(?device, "using device for Kyutai TTS");

        let lm_config = moshi::lm::Config::tts_202501();
        let audio_codebooks = lm_config.audio_codebooks;
        let dtype = DType::BF16;

        tracing::info!("loading Mimi audio tokenizer");
        let mimi_path_str = mimi_path.to_string_lossy().to_string();
        let audio_tokenizer = moshi::mimi::load(&mimi_path_str, Some(audio_codebooks), &device)?;

        tracing::info!("loading SentencePiece text tokenizer");
        let text_tokenizer = sentencepiece::SentencePieceProcessor::open(&tokenizer_path)?;

        tracing::info!("loading LM weights");
        let lm_path_str = lm_path.to_string_lossy().to_string();
        let vb_lm = unsafe {
            VarBuilder::from_mmaped_safetensors(&[lm_path_str.as_str()], dtype, &device)?
        };
        let lm =
            moshi::lm::LmModel::new(&lm_config, moshi::nn::MaybeQuantizedVarBuilder::Real(vb_lm))?;

        tracing::info!("loading voice conditioning");
        let voice_path_str = voice_path.to_string_lossy().to_string();
        let voice_tensors = candle_core::safetensors::load(&voice_path_str, &device)?;
        let ca_src = voice_tensors
            .get("ca_src")
            .ok_or_else(|| anyhow!("missing ca_src tensor in voice file"))?
            .clone();
        let ca_src = ca_src.narrow(0, 0, 1)?.to_dtype(dtype)?;

        let tts_config = moshi::tts_streaming::Config::v202501();

        tracing::info!("Kyutai TTS model ready");
        Ok(Self {
            lm,
            audio_tokenizer,
            text_tokenizer,
            tts_config,
            ca_src,
            device,
        })
    }

    /// Stream synthesized 24kHz mono f32 LE PCM chunks for `text`.
    /// `on_chunk` is called for each ~80ms frame of audio as it's generated.
    /// Runs synchronously; call from `tokio::task::spawn_blocking`.
    pub fn speak(&mut self, text: &str, mut on_chunk: impl FnMut(Vec<u8>)) -> Result<()> {
        use moshi::tts_streaming::AllowedTokens;

        let tts_config = self.tts_config.clone();
        let dtype = DType::BF16;

        // Tokenize text: BOS + word-piece tokens
        let mut all_tokens: Vec<u32> = vec![tts_config.text_bos_token];
        for word in text.split_whitespace() {
            let pieces = self.text_tokenizer.encode(word)?;
            all_tokens.extend(pieces.into_iter().map(|p| p.id as u32));
        }

        // Build streaming inference state
        let audio_lp = LogitsProcessor::from_sampling(42, Sampling::ArgMax);
        let text_lp = LogitsProcessor::from_sampling(42, Sampling::ArgMax);
        let mut state = moshi::tts_streaming::State::new(
            self.lm.clone(),
            Some(moshi::transformer::CaSrc::Tokens(self.ca_src.clone())),
            2048,
            audio_lp,
            text_lp,
            None,
            tts_config.clone(),
        );

        let text_audio_delay = tts_config.text_audio_delay_in_tokens;
        let acoustic_delay = tts_config.acoustic_delay;
        let extra_steps = tts_config.extra_steps;
        let text_eop = tts_config.text_eop_token;
        let text_pad = tts_config.text_pad_token;
        let mut last_text_token = tts_config.text_start_token;

        let mut token_idx = 0usize;
        let mut past_end = 0usize;
        let mut at_end = false;

        let mut audio_tokenizer = self.audio_tokenizer.clone();
        audio_tokenizer.reset_state();

        for step_idx in 0..2048usize {
            let allowed = if at_end {
                past_end += 1;
                if past_end > extra_steps + text_audio_delay {
                    break;
                }
                AllowedTokens::Pad
            } else if token_idx >= all_tokens.len() {
                AllowedTokens::PadOrEpad
            } else {
                AllowedTokens::Text(all_tokens[token_idx])
            };

            last_text_token = state.step(last_text_token, allowed, None)?;

            if last_text_token == text_eop {
                token_idx = all_tokens.len();
            } else if last_text_token != text_pad && !at_end {
                token_idx += 1;
                if token_idx >= all_tokens.len() {
                    at_end = true;
                }
            }

            if let Some(audio_tokens) = state.last_audio_tokens() {
                if step_idx >= text_audio_delay + acoustic_delay {
                    let cb = audio_tokens.len();
                    let audio_tensor =
                        Tensor::from_vec(audio_tokens, (1usize, cb, 1usize), state.device())?;
                    let pcm = audio_tokenizer.decode_step(&audio_tensor.into(), &().into())?;
                    if let Some(pcm_tensor) = pcm.as_option() {
                        let samples: Vec<f32> = pcm_tensor.flatten_all()?.to_vec1()?;
                        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
                        on_chunk(bytes);
                    }
                }
            }
        }

        Ok(())
    }
}
