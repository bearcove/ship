import { useEffect, useMemo, useState } from "react";
import { Box, Code, IconButton } from "@radix-ui/themes";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { bundledLanguages, codeToHtml } from "shiki";
import type { BundledLanguage } from "shiki";
import { Check, CircleNotch, CopySimple, SpeakerHigh } from "@phosphor-icons/react";
import type { ContentBlock } from "../../generated/ship";
import { fixMarkdownBackticks } from "../../utils/fixMarkdownBackticks";
import {
  bubbleActions,
  bubbleContent,
  textBlockCodeBlock,
  textBlockCodeFallback,
  textBlockRoot,
} from "../../styles/session-view.css";
import { spinAnimation } from "../../styles/global.css";
import { usePlayback } from "../../context/PlaybackContext";

type TextBlockType = Extract<ContentBlock, { tag: "Text" }>;

interface BubbleActionsProps {
  block: TextBlockType;
  speakable?: boolean;
  isLast?: boolean;
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

export function MarkdownCodeBlock({ className, code }: { className?: string; code: string }) {
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
      .catch(() => { });

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

export function BubbleActions({ block, speakable, isLast }: BubbleActionsProps) {
  const [copied, setCopied] = useState(false);
  const playback = usePlayback();

  const handleCopy = () => {
    void navigator.clipboard.writeText(block.text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  const isThisActive = playback.state !== "idle" && playback.activeText === block.text;
  const isBusy = playback.state !== "idle";

  return (
    <div className={bubbleActions}>
      {speakable && (
        <IconButton
          size="2"
          variant="ghost"
          onClick={() => isThisActive ? playback.stop() : playback.speak(block.text)}
          aria-label={isThisActive ? "Stop" : "Speak"}
          disabled={isBusy && !isThisActive}
        >
          {isThisActive ? (
            <CircleNotch size={16} style={{ animation: `${spinAnimation} 2.5s linear infinite` }} />
          ) : (
            <SpeakerHigh size={16} />
          )}
        </IconButton>
      )}
      <IconButton size="2" variant="ghost" onClick={handleCopy} aria-label="Copy">
        {copied ? <Check size={16} /> : <CopySimple size={16} />}
      </IconButton>
    </div>
  );
}

// r[ui.block.text]
export function TextBlock({ block }: { block: TextBlockType }) {
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
    <Box className={textBlockRoot}>
      <div className={bubbleContent}>
        <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
          {fixMarkdownBackticks(block.text)}
        </ReactMarkdown>
      </div>
    </Box>
  );
}
