import { createContext, useCallback, useContext, useMemo, useRef, useState } from "react";
import { channel } from "@bearcove/roam-core";
import { getShipClient } from "../api/client";

type PlaybackState = "idle" | "loading" | "playing";

interface PlaybackContextValue {
  state: PlaybackState;
  activeText: string | null;
  analyser: AnalyserNode | null;
  speak: (text: string) => void;
  stop: () => void;
}

const PlaybackContext = createContext<PlaybackContextValue>(null!);

export function PlaybackProvider({ children }: { children: React.ReactNode }) {
  const [state, setState] = useState<PlaybackState>("idle");
  const [activeText, setActiveText] = useState<string | null>(null);
  const [analyser, setAnalyser] = useState<AnalyserNode | null>(null);
  const sourcesRef = useRef<AudioBufferSourceNode[]>([]);
  const ctxRef = useRef<AudioContext | null>(null);

  const cleanup = useCallback(() => {
    for (const src of sourcesRef.current) {
      try {
        src.stop();
      } catch {
        // already stopped
      }
      src.disconnect();
    }
    sourcesRef.current = [];
    if (ctxRef.current) {
      void ctxRef.current.close();
      ctxRef.current = null;
    }
    setAnalyser(null);
    setActiveText(null);
    setState("idle");
  }, []);

  const stop = useCallback(() => {
    cleanup();
  }, [cleanup]);

  const speak = useCallback(
    (text: string) => {
      if (state !== "idle") return;

      // Create AudioContext immediately in the user gesture handler
      // so it isn't blocked on mobile browsers (iOS Safari requires this)
      const audioCtx = new AudioContext({ sampleRate: 24000 });
      ctxRef.current = audioCtx;
      setActiveText(text);
      setState("loading");

      const analyserNode = audioCtx.createAnalyser();
      analyserNode.fftSize = 256;
      analyserNode.connect(audioCtx.destination);

      void (async () => {
        try {
          const client = await getShipClient();
          const [tx, rx] = channel<Uint8Array>();

          const callPromise = client.speakText(text, tx);

          let nextStartTime = audioCtx.currentTime;
          let chunkCount = 0;
          let lastSource: AudioBufferSourceNode | null = null;

          while (true) {
            const chunk = await rx.recv();
            if (chunk === null) break;

            // Bail if user called stop
            if (ctxRef.current !== audioCtx) return;

            // Resume context if suspended (needed for mobile) — only on first chunk
            if (chunkCount === 0 && audioCtx.state === "suspended") {
              await audioCtx.resume();
            }

            // Decode chunk: 24kHz mono f32 little-endian PCM
            const sampleCount = chunk.length / 4;
            const samples = new Float32Array(sampleCount);
            const view = new DataView(chunk.buffer, chunk.byteOffset, chunk.byteLength);
            for (let i = 0; i < sampleCount; i++) {
              samples[i] = view.getFloat32(i * 4, true);
            }

            const buffer = audioCtx.createBuffer(1, sampleCount, 24000);
            buffer.copyToChannel(samples, 0);

            const src = audioCtx.createBufferSource();
            src.buffer = buffer;
            src.connect(analyserNode);
            sourcesRef.current.push(src);
            lastSource = src;

            // Schedule this chunk to play right after the previous one
            src.start(nextStartTime);
            nextStartTime += buffer.duration;

            if (chunkCount === 0) {
              setAnalyser(analyserNode);
              setState("playing");
            }
            chunkCount++;
          }

          await callPromise;

          // Bail if user called stop while we were receiving
          if (ctxRef.current !== audioCtx) return;

          if (chunkCount === 0) {
            console.warn("speak_text: no audio received");
            cleanup();
            return;
          }

          // When the last scheduled source finishes, clean up
          if (lastSource) {
            lastSource.onended = () => {
              if (ctxRef.current === audioCtx) {
                cleanup();
              }
            };
          }
        } catch (err) {
          console.error("speak_text failed:", err);
          cleanup();
        }
      })();
    },
    [state, cleanup],
  );

  const value = useMemo(
    () => ({ state, activeText, analyser, speak, stop }),
    [state, activeText, analyser, speak, stop],
  );

  return <PlaybackContext.Provider value={value}>{children}</PlaybackContext.Provider>;
}

export function usePlayback() {
  return useContext(PlaybackContext);
}
