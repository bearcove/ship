import { createContext, useCallback, useContext, useRef, useState } from "react";
import { channel } from "@bearcove/roam-core";
import { getShipClient } from "../api/client";

type PlaybackState = "idle" | "loading" | "playing";

interface PlaybackContextValue {
  state: PlaybackState;
  analyser: AnalyserNode | null;
  speak: (text: string) => void;
  stop: () => void;
}

const PlaybackContext = createContext<PlaybackContextValue>(null!);

export function PlaybackProvider({ children }: { children: React.ReactNode }) {
  const [state, setState] = useState<PlaybackState>("idle");
  const [analyser, setAnalyser] = useState<AnalyserNode | null>(null);
  const sourceRef = useRef<AudioBufferSourceNode | null>(null);
  const ctxRef = useRef<AudioContext | null>(null);

  const cleanup = useCallback(() => {
    sourceRef.current?.stop();
    sourceRef.current?.disconnect();
    sourceRef.current = null;
    if (ctxRef.current) {
      void ctxRef.current.close();
      ctxRef.current = null;
    }
    setAnalyser(null);
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
      setState("loading");

      void (async () => {
        try {
          const client = await getShipClient();
          const [tx, rx] = channel<Uint8Array>();

          const callPromise = client.speakText(text, tx);

          const chunks: Uint8Array[] = [];
          while (true) {
            const chunk = await rx.recv();
            if (chunk === null) break;
            chunks.push(chunk);
          }

          await callPromise;

          // Check if we were stopped while loading
          if (ctxRef.current !== audioCtx) return;

          if (chunks.length === 0) {
            console.warn("speak_text: no audio received");
            cleanup();
            return;
          }

          const totalBytes = chunks.reduce((sum, c) => sum + c.length, 0);
          const allBytes = new Uint8Array(totalBytes);
          let offset = 0;
          for (const chunk of chunks) {
            allBytes.set(chunk, offset);
            offset += chunk.length;
          }

          const sampleCount = allBytes.length / 4;
          const samples = new Float32Array(sampleCount);
          const view = new DataView(allBytes.buffer, allBytes.byteOffset, allBytes.byteLength);
          for (let i = 0; i < sampleCount; i++) {
            samples[i] = view.getFloat32(i * 4, true);
          }

          // Resume context if suspended (needed for mobile)
          if (audioCtx.state === "suspended") {
            await audioCtx.resume();
          }

          const buffer = audioCtx.createBuffer(1, samples.length, 24000);
          buffer.copyToChannel(samples, 0);

          const analyserNode = audioCtx.createAnalyser();
          analyserNode.fftSize = 256;

          const src = audioCtx.createBufferSource();
          src.buffer = buffer;
          src.connect(analyserNode);
          analyserNode.connect(audioCtx.destination);
          sourceRef.current = src;
          setAnalyser(analyserNode);
          setState("playing");

          src.onended = () => {
            // Only cleanup if this is still the active source
            if (sourceRef.current === src) {
              cleanup();
            }
          };
          src.start();
        } catch (err) {
          console.error("speak_text failed:", err);
          cleanup();
        }
      })();
    },
    [state, cleanup],
  );

  return (
    <PlaybackContext.Provider value={{ state, analyser, speak, stop }}>
      {children}
    </PlaybackContext.Provider>
  );
}

export function usePlayback() {
  return useContext(PlaybackContext);
}
