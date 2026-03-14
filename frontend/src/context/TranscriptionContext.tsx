import { createContext, useCallback, useContext, useMemo, useRef, useState } from "react";
import { channel, type Tx } from "@bearcove/roam-core";
import { getShipClient } from "../api/client";
import type { TranscribeMessage, TranscribeSegment } from "../generated/ship";

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

interface TranscriptionContextValue {
  state: TranscriptionState;
  result: TranscriptionResult | null;
  analyser: AnalyserNode | null;
  targetSessionId: string | null;
  sendAfterTranscription: boolean;
  startRecording(sessionId: string): void;
  stopRecording(): void;
  stopAndSend(): void;
  cancelRecording(): void;
  clearResult(): void;
  isRecording(): boolean;
}

const TranscriptionContext = createContext<TranscriptionContextValue>(null!);

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

export function TranscriptionProvider({ children }: { children: React.ReactNode }) {
  const [state, setState] = useState<TranscriptionState>({ tag: "idle" });
  const [result, setResult] = useState<TranscriptionResult | null>(null);
  const [analyser, setAnalyser] = useState<AnalyserNode | null>(null);
  const [targetSessionId, setTargetSessionId] = useState<string | null>(null);
  const [sendAfterTranscription, setSendAfterTranscription] = useState(false);
  const activeRef = useRef<{
    audioContext: AudioContext;
    mediaStream: MediaStream;
    audioTx: Tx<Uint8Array>;
    stopElapsedTimer: () => void;
    flushAudio: () => void;
  } | null>(null);

  const startRecording = useCallback((sessionId: string) => {
    if (activeRef.current) return;
    setResult(null);
    setTargetSessionId(sessionId);
    setSendAfterTranscription(false);

    void (async () => {
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
        const [segTx, segRx] = channel<TranscribeMessage>();

        // Start the RPC call
        const client = await getShipClient();
        const callPromise = client.transcribeAudio(audioRx, segTx);

        // Buffer audio samples and send in ~100ms batches to avoid flooding
        // the WebSocket (AudioWorklet fires every ~2.67ms at 48kHz)
        const sourceSampleRate = audioContext.sampleRate;
        const BATCH_INTERVAL_MS = 100;
        let pendingSamples: Float32Array[] = [];
        let pendingLength = 0;

        const flushAudio = () => {
          if (pendingLength === 0) return;
          const merged = new Float32Array(pendingLength);
          let offset = 0;
          for (const chunk of pendingSamples) {
            merged.set(chunk, offset);
            offset += chunk.length;
          }
          pendingSamples = [];
          pendingLength = 0;

          const resampled = resample(merged, sourceSampleRate);
          const bytes = float32ToBytes(resampled);
          const copy = new Uint8Array(bytes.length);
          copy.set(bytes);
          audioTx.send(copy).catch(() => {});
        };

        const batchTimer = setInterval(flushAudio, BATCH_INTERVAL_MS);

        workletNode.port.onmessage = (e: MessageEvent<Float32Array>) => {
          pendingSamples.push(new Float32Array(e.data));
          pendingLength += e.data.length;
        };

        const analyserNode = audioContext.createAnalyser();
        analyserNode.fftSize = 256;
        analyserNode.smoothingTimeConstant = 0.7;
        source.connect(analyserNode);

        source.connect(workletNode);
        workletNode.connect(audioContext.destination);

        setAnalyser(analyserNode);

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
          flushAudio: () => {
            clearInterval(batchTimer);
            flushAudio();
          },
        };

        // Background: receive segments in real-time.
        void (async () => {
          const allSegments: TranscribeSegment[] = [];
          let gotError = false;
          while (true) {
            const msg = await segRx.recv();
            if (msg === null) break;
            if (msg.tag === "Error") {
              setState({ tag: "error", message: msg.message });
              gotError = true;
              break;
            }
            allSegments.push(msg.value);
            const fullText = allSegments.map((s) => s.text.trim()).join(" ");
            setResult({ text: fullText, segments: [...allSegments] });
          }
          await callPromise;
          if (!gotError) {
            setState({ tag: "idle" });
          }
        })();
      } catch (err) {
        setState({
          tag: "error",
          message: err instanceof Error ? err.message : String(err),
        });
      }
    })();
  }, []);

  const doStop = useCallback(async () => {
    const active = activeRef.current;
    if (!active) return;
    activeRef.current = null;
    setAnalyser(null);

    active.stopElapsedTimer();
    active.flushAudio();

    // Stop the mic
    for (const track of active.mediaStream.getTracks()) {
      track.stop();
    }

    // Close the audio channel to signal the server to process
    active.audioTx.close();
    await active.audioContext.close();

    setState({ tag: "processing" });

    // Safety timeout: if still "processing" after 15s, transition to error
    setTimeout(() => {
      setState((prev) =>
        prev.tag === "processing"
          ? { tag: "error", message: "Transcription timed out" }
          : prev,
      );
    }, 15_000);
  }, []);

  const stopRecording = useCallback(() => {
    void doStop();
  }, [doStop]);

  const stopAndSend = useCallback(() => {
    setSendAfterTranscription(true);
    void doStop();
  }, [doStop]);

  const cancelRecording = useCallback(() => {
    const active = activeRef.current;
    if (!active) return;
    activeRef.current = null;
    setAnalyser(null);

    active.stopElapsedTimer();

    for (const track of active.mediaStream.getTracks()) {
      track.stop();
    }

    active.audioTx.close();
    void active.audioContext.close();

    setState({ tag: "idle" });
    setTargetSessionId(null);
    setSendAfterTranscription(false);
    setResult(null);
  }, []);

  const clearResult = useCallback(() => {
    setResult(null);
    setTargetSessionId(null);
    setSendAfterTranscription(false);
  }, []);

  const isRecording = useCallback(() => {
    return activeRef.current !== null;
  }, []);

  const value = useMemo(
    () => ({
      state,
      result,
      analyser,
      targetSessionId,
      sendAfterTranscription,
      startRecording,
      stopRecording,
      stopAndSend,
      cancelRecording,
      clearResult,
      isRecording,
    }),
    [
      state,
      result,
      analyser,
      targetSessionId,
      sendAfterTranscription,
      startRecording,
      stopRecording,
      stopAndSend,
      cancelRecording,
      clearResult,
      isRecording,
    ],
  );

  return <TranscriptionContext.Provider value={value}>{children}</TranscriptionContext.Provider>;
}

export function useTranscription() {
  return useContext(TranscriptionContext);
}
