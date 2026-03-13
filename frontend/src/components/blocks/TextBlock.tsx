import { useEffect, useMemo, useState } from "react";
import { Box, Code, IconButton } from "@radix-ui/themes";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { bundledLanguages, codeToHtml } from "shiki";
import type { BundledLanguage } from "shiki";
import { Check, CircleNotch, CopySimple, SpeakerHigh } from "@phosphor-icons/react";
import { channel } from "@bearcove/roam-core";
import type { ContentBlock } from "../../generated/ship";
import { getShipClient } from "../../api/client";
import {
  bubbleActions,
  bubbleContent,
  textBlockCodeBlock,
  textBlockCodeFallback,
  textBlockRoot,
} from "../../styles/session-view.css";
import { spinAnimation } from "../../styles/global.css";

type TextBlockType = Extract<ContentBlock, { tag: "Text" }>;

interface Props {
  block: TextBlockType;
  speakable?: boolean;
}

function useColorScheme(): "dark" | "light" {
  const [scheme, setScheme] = useState<"dark" | "light">(() =>
    window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light",
  );

  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => setScheme(e.matches ? "dark" : "light");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  return scheme;
}

function resolveLanguage(className?: string): BundledLanguage | null {
  const match = /language-([^\s]+)/.exec(className ?? "");
  const language = match?.[1]?.toLowerCase();
  if (!language) return null;
  return language in bundledLanguages ? (language as BundledLanguage) : null;
}

function MarkdownCodeBlock({ className, code }: { className?: string; code: string }) {
  const [highlightedHtml, setHighlightedHtml] = useState<string | null>(null);
  const language = resolveLanguage(className);
  const colorScheme = useColorScheme();
  const shikiTheme = colorScheme === "dark" ? "github-dark" : "github-light";

  useEffect(() => {
    let cancelled = false;

    if (!language) return () => void 0;

    void codeToHtml(code, {
      lang: language,
      theme: shikiTheme,
      rootStyle: "background-color: transparent",
    })
      .then((html) => {
        if (!cancelled) setHighlightedHtml(html);
      })
      .catch(() => {});

    return () => {
      cancelled = true;
    };
  }, [code, language, shikiTheme]);

  if (highlightedHtml) {
    return (
      <Box className={textBlockCodeBlock} dangerouslySetInnerHTML={{ __html: highlightedHtml }} />
    );
  }

  return (
    <pre className={textBlockCodeFallback}>
      <code>{code}</code>
    </pre>
  );
}

type SpeakState = "idle" | "loading" | "playing";

// r[ui.block.text]
export function TextBlock({ block, speakable }: Props) {
  const [copied, setCopied] = useState(false);
  const [showActions, setShowActions] = useState(false);
  const [speakState, setSpeakState] = useState<SpeakState>("idle");

  const handleCopy = () => {
    void navigator.clipboard.writeText(block.text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  const handleSpeak = async () => {
    if (speakState !== "idle") return;
    setSpeakState("loading");

    try {
      const client = await getShipClient();
      const [tx, rx] = channel<Uint8Array>();

      const callPromise = client.speakText(block.text, tx);

      // Accumulate all byte chunks until channel closes
      const chunks: Uint8Array[] = [];
      while (true) {
        const chunk = await rx.recv();
        if (chunk === null) break;
        chunks.push(chunk);
      }

      await callPromise;

      if (chunks.length === 0) {
        console.warn("speak_text: no audio received");
        setSpeakState("idle");
        return;
      }

      // Concatenate chunks
      const totalBytes = chunks.reduce((sum, c) => sum + c.length, 0);
      const allBytes = new Uint8Array(totalBytes);
      let offset = 0;
      for (const chunk of chunks) {
        allBytes.set(chunk, offset);
        offset += chunk.length;
      }

      // Decode f32 LE samples
      const sampleCount = allBytes.length / 4;
      const samples = new Float32Array(sampleCount);
      const view = new DataView(allBytes.buffer, allBytes.byteOffset, allBytes.byteLength);
      for (let i = 0; i < sampleCount; i++) {
        samples[i] = view.getFloat32(i * 4, true);
      }

      // Play via Web Audio API at 24kHz
      setSpeakState("playing");
      const ctx = new AudioContext({ sampleRate: 24000 });
      const buffer = ctx.createBuffer(1, samples.length, 24000);
      buffer.copyToChannel(samples, 0);
      const src = ctx.createBufferSource();
      src.buffer = buffer;
      src.connect(ctx.destination);

      await new Promise<void>((resolve) => {
        src.onended = () => resolve();
        src.start();
      });

      await ctx.close();
      setSpeakState("idle");
    } catch (err) {
      console.error("speak_text failed:", err);
      setSpeakState("idle");
    }
  };

  const markdownComponents = useMemo(
    () => ({
      code({ children, className }: { children?: React.ReactNode; className?: string }) {
        const rawCode = String(children ?? "");
        const isBlock =
          Boolean(className?.startsWith("language-")) ||
          rawCode.includes("\n") ||
          rawCode.endsWith("\n");
        const code = rawCode.replace(/\n$/, "");
        if (isBlock) {
          return <MarkdownCodeBlock className={className} code={code} />;
        }
        return <Code size="1">{children}</Code>;
      },
    }),
    [],
  );

  return (
    <Box
      className={textBlockRoot}
      data-show-actions={showActions ? "true" : undefined}
      onClick={() => setShowActions((v) => !v)}
    >
      <div className={bubbleContent}>
        <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
          {block.text}
        </ReactMarkdown>
      </div>
      <div className={bubbleActions}>
        {speakable && (
          <IconButton
            size="1"
            variant="ghost"
            onClick={() => void handleSpeak()}
            aria-label="Speak"
            disabled={speakState !== "idle"}
          >
            {speakState === "idle" ? (
              <SpeakerHigh size={12} />
            ) : (
              <CircleNotch size={12} style={{ animation: `${spinAnimation} 1s linear infinite` }} />
            )}
          </IconButton>
        )}
        <IconButton size="1" variant="ghost" onClick={handleCopy} aria-label="Copy">
          {copied ? <Check size={12} /> : <CopySimple size={12} />}
        </IconButton>
      </div>
    </Box>
  );
}
