import { useEffect, useState } from "react";
import { Box, Code } from "@radix-ui/themes";
import ReactMarkdown from "react-markdown";
import { bundledLanguages, codeToHtml } from "shiki";
import type { BundledLanguage } from "shiki";
import type { ContentBlock } from "../../generated/ship";
import {
  textBlockCodeBlock,
  textBlockCodeFallback,
  textBlockRoot,
} from "../../styles/session-view.css";

type TextBlockType = Extract<ContentBlock, { tag: "Text" }>;

interface Props {
  block: TextBlockType;
}

const shikiTheme = "github-dark";

function resolveLanguage(className?: string): BundledLanguage | null {
  const match = /language-([^\s]+)/.exec(className ?? "");
  const language = match?.[1]?.toLowerCase();
  if (!language) return null;
  return language in bundledLanguages ? (language as BundledLanguage) : null;
}

function MarkdownCodeBlock({ className, code }: { className?: string; code: string }) {
  const [highlightedHtml, setHighlightedHtml] = useState<string | null>(null);
  const language = resolveLanguage(className);

  useEffect(() => {
    let cancelled = false;
    setHighlightedHtml(null);

    if (!language) return () => void 0;

    void codeToHtml(code, { lang: language, theme: shikiTheme })
      .then((html) => {
        if (!cancelled) setHighlightedHtml(html);
      })
      .catch(() => {
        if (!cancelled) setHighlightedHtml(null);
      });

    return () => {
      cancelled = true;
    };
  }, [code, language]);

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

// r[ui.block.text]
export function TextBlock({ block }: Props) {
  return (
    <Box className={textBlockRoot}>
      <ReactMarkdown
        components={{
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
        }}
      >
        {block.text}
      </ReactMarkdown>
    </Box>
  );
}
