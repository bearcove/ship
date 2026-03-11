import { useCallback, useRef, useState } from "react";
import { channel, type Tx } from "@bearcove/roam-core";
import { getShipClient } from "../api/client";
import type { TranscribeSegment } from "../generated/ship";

const TARGET_SAMPLE_RATE = 16000;

export type TranscriptionState =
  | { tag: "idle" }
  | { tag: "recording"; elapsed: number }
  | { tag: "processing" }
  | { tag: "error"; message: string };

export interface TranscriptionResult {
  text: string;
  segments: TranscribeSegment[];
}

/** Downsample from sourceSampleRate to 16kHz mono. */
function resample(samples: Float32Array, sourceSampleRate: number): Float32Array {
  if (sourceSampleRate === TARGET_SAMPLE_RATE) return samples;
  const ratio = sourceSampleRate / TARGET_SAMPLE_RATE;
  const outLength = Math.floor(samples.length / ratio);
  const out = new Float32Array(outLength);
  for (let i = 0; i < outLength; i++) {
    out[i] = samples[Math.floor(i * ratio)];
  }
  return out;
}

/** Convert Float32Array to Uint8Array (little-endian f32 bytes). */
function float32ToBytes(samples: Float32Array): Uint8Array {
  return new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength);
}

export function useTranscription() {
  const [state, setState] = useState<TranscriptionState>({ tag: "idle" });
  const [result, setResult] = useState<TranscriptionResult | null>(null);
  const activeRef = useRef<{
    audioContext: AudioContext;
    mediaStream: MediaStream;
    audioTx: Tx<Uint8Array>;
    stopElapsedTimer: () => void;
  } | null>(null);

  const startRecording = useCallback(async () => {
    if (activeRef.current) return;
    setResult(null);

    try {
      const mediaStream = await navigator.mediaDevices.getUserMedia({
        audio: {
          channelCount: 1,
          sampleRate: { ideal: TARGET_SAMPLE_RATE },
          echoCancellation: true,
          noiseSuppression: true,
        },
      });

      const audioContext = new AudioContext({ sampleRate: undefined });
      await audioContext.audioWorklet.addModule("/audio-capture-processor.js");

      const source = audioContext.createMediaStreamSource(mediaStream);
      const workletNode = new AudioWorkletNode(audioContext, "audio-capture-processor");

      // Create roam channels
      const [audioTx, audioRx] = channel<Uint8Array>();
      const [segTx, segRx] = channel<TranscribeSegment>();

      // Start the RPC call
      const client = await getShipClient();
      const callPromise = client.transcribeAudio(audioRx, segTx);

      // Forward audio samples from worklet to roam channel
      const sourceSampleRate = audioContext.sampleRate;
      workletNode.port.onmessage = (e: MessageEvent<Float32Array>) => {
        const resampled = resample(e.data, sourceSampleRate);
        const bytes = float32ToBytes(resampled);
        // Copy the bytes since the underlying buffer may be reused
        const copy = new Uint8Array(bytes.length);
        copy.set(bytes);
        audioTx.send(copy).catch(() => {
          // Channel closed, stop recording
        });
      };

      source.connect(workletNode);
      workletNode.connect(audioContext.destination);

      // Elapsed timer
      const startTime = Date.now();
      const elapsedInterval = setInterval(() => {
        setState({ tag: "recording", elapsed: Date.now() - startTime });
      }, 100);

      setState({ tag: "recording", elapsed: 0 });

      activeRef.current = {
        audioContext,
        mediaStream,
        audioTx,
        stopElapsedTimer: () => clearInterval(elapsedInterval),
      };

      // Background: wait for segments from the server
      void (async () => {
        const segments: TranscribeSegment[] = [];
        while (true) {
          const seg = await segRx.recv();
          if (seg === null) break;
          segments.push(seg);
        }
        await callPromise;

        const text = segments
          .map((s) => s.text)
          .join("")
          .trim();
        setResult({ text, segments });
        setState({ tag: "idle" });
      })();
    } catch (err) {
      setState({
        tag: "error",
        message: err instanceof Error ? err.message : String(err),
      });
    }
  }, []);

  const stopRecording = useCallback(async () => {
    const active = activeRef.current;
    if (!active) return;
    activeRef.current = null;

    active.stopElapsedTimer();

    // Stop the mic
    for (const track of active.mediaStream.getTracks()) {
      track.stop();
    }

    // Close the audio channel to signal the server to process
    active.audioTx.close();
    await active.audioContext.close();

    setState({ tag: "processing" });
  }, []);

  const cancelRecording = useCallback(() => {
    const active = activeRef.current;
    if (!active) return;
    activeRef.current = null;

    active.stopElapsedTimer();

    for (const track of active.mediaStream.getTracks()) {
      track.stop();
    }

    active.audioTx.close();
    void active.audioContext.close();

    setState({ tag: "idle" });
    setResult(null);
  }, []);

  return {
    state,
    result,
    startRecording,
    stopRecording,
    cancelRecording,
    clearResult: useCallback(() => setResult(null), []),
  };
}
