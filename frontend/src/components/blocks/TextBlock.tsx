import { useEffect, useMemo, useRef, useState } from "react";
import { Box, Button, Code, IconButton } from "@radix-ui/themes";
import ReactMarkdown from "react-markdown";
import { bundledLanguages, codeToHtml } from "shiki";
import type { BundledLanguage } from "shiki";
import { Check, CopySimple } from "@phosphor-icons/react";
import type { ContentBlock } from "../../generated/ship";
import {
  bubbleContent,
  bubbleContentCollapsed,
  bubbleCopyBtn,
  textBlockCodeBlock,
  textBlockCodeFallback,
  textBlockRoot,
} from "../../styles/session-view.css";

type TextBlockType = Extract<ContentBlock, { tag: "Text" }>;

interface Props {
  block: TextBlockType;
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

    void codeToHtml(code, { lang: language, theme: shikiTheme })
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

const COLLAPSE_HEIGHT = 400;

// r[ui.block.text]
export function TextBlock({ block }: Props) {
  const [isOverflowing, setIsOverflowing] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [copied, setCopied] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = contentRef.current;
    if (!el) return;
    const check = () => setIsOverflowing(el.scrollHeight > COLLAPSE_HEIGHT);
    check();
    const observer = new ResizeObserver(check);
    observer.observe(el);
    return () => observer.disconnect();
  }, [block.text]);

  const handleCopy = () => {
    void navigator.clipboard.writeText(block.text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  const collapsed = isOverflowing && !isExpanded;

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
      <div
        ref={contentRef}
        className={collapsed ? `${bubbleContent} ${bubbleContentCollapsed}` : bubbleContent}
      >
        <ReactMarkdown components={markdownComponents}>{block.text}</ReactMarkdown>
      </div>
      {isOverflowing && (
        <Button
          size="1"
          variant="ghost"
          style={{ marginTop: "var(--space-1)" }}
          onClick={() => setIsExpanded(!isExpanded)}
        >
          {isExpanded ? "Show less" : "Show more"}
        </Button>
      )}
      <IconButton
        size="1"
        variant="ghost"
        className={bubbleCopyBtn}
        onClick={handleCopy}
        aria-label="Copy"
      >
        {copied ? <Check size={12} /> : <CopySimple size={12} />}
      </IconButton>
    </Box>
  );
}
