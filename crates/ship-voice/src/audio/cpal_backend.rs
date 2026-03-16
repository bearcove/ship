use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::{AudioHost, AudioStream};

pub struct CpalAudioHost;

struct CpalStream(cpal::Stream);

// cpal::Stream is !Send, but the stream callback runs on its own thread
// and we only hold it for the purpose of keeping it alive / stopping it.
// SAFETY: we never access the stream from multiple threads — we only drop it.
unsafe impl Send for CpalStream {}

impl AudioStream for CpalStream {
    fn stop(self: Box<Self>) {
        drop(self.0);
    }
}

impl AudioHost for CpalAudioHost {
    fn play_samples(
        &self,
        samples: Arc<[f32]>,
        sample_rate: u32,
    ) -> anyhow::Result<(Box<dyn AudioStream>, tokio::sync::oneshot::Receiver<()>)> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("no output device"))?;
        let config = cpal::StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let pos = Arc::new(AtomicUsize::new(0));
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
        let done_tx = std::sync::Mutex::new(Some(done_tx));

        let samples2 = Arc::clone(&samples);
        let pos2 = Arc::clone(&pos);
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                let p = pos2.load(Ordering::Relaxed);
                let n = data.len().min(samples2.len().saturating_sub(p));
                data[..n].copy_from_slice(&samples2[p..p + n]);
                data[n..].fill(0.0);
                pos2.fetch_add(n, Ordering::Relaxed);
                if p + n >= samples2.len()
                    && let Some(tx) = done_tx.lock().expect("done_tx mutex poisoned").take()
                {
                    let _ = tx.send(());
                }
            },
            |err| tracing::error!("cpal output error: {err}"),
            None,
        )?;
        stream.play()?;

        Ok((Box::new(CpalStream(stream)), done_rx))
    }

    fn capture_mic(
        &self,
    ) -> anyhow::Result<(
        Box<dyn AudioStream>,
        tokio::sync::mpsc::UnboundedReceiver<Vec<f32>>,
    )> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("no default input device"))?;
        tracing::info!(
            device = device.name().unwrap_or_default(),
            "using input device"
        );

        let default_config = device.default_input_config()?;
        let native_sample_rate = default_config.sample_rate().0;
        let native_channels = default_config.channels();
        tracing::info!(
            sample_rate = native_sample_rate,
            channels = native_channels,
            "native input config"
        );

        let config = cpal::StreamConfig {
            channels: native_channels,
            sample_rate: default_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        let (audio_tx, audio_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<f32>>();

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mono: Vec<f32> = if native_channels == 1 {
                    data.to_vec()
                } else {
                    data.chunks_exact(native_channels as usize)
                        .map(|frame| frame.iter().sum::<f32>() / native_channels as f32)
                        .collect()
                };

                if native_sample_rate == 16000 {
                    let _ = audio_tx.send(mono);
                } else {
                    let ratio = 16000.0 / native_sample_rate as f64;
                    let out_len = (mono.len() as f64 * ratio) as usize;
                    let mut resampled = Vec::with_capacity(out_len);
                    for i in 0..out_len {
                        let src_idx = i as f64 / ratio;
                        let idx = src_idx as usize;
                        let frac = src_idx - idx as f64;
                        let sample = if idx + 1 < mono.len() {
                            mono[idx] as f64 * (1.0 - frac) + mono[idx + 1] as f64 * frac
                        } else if idx < mono.len() {
                            mono[idx] as f64
                        } else {
                            0.0
                        };
                        resampled.push(sample as f32);
                    }
                    let _ = audio_tx.send(resampled);
                }
            },
            |err| {
                tracing::error!("audio input error: {err}");
            },
            None,
        )?;
        stream.play()?;
        tracing::info!("listening...");

        Ok((Box::new(CpalStream(stream)), audio_rx))
    }
}
