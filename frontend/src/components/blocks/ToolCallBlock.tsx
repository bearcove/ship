import { Fragment, useMemo, useState } from "react";
import { Badge, Box, Code, Flex, ScrollArea, Spinner, Text } from "@radix-ui/themes";
import { CaretDown, CaretRight } from "@phosphor-icons/react";
import ReactMarkdown from "react-markdown";
import type { ContentBlock, ToolCallContent, ToolCallLocation } from "../../generated/ship";
import { formatDisplayPath, formatDisplayText } from "../../utils/displayPath";
import {
  diffAdd,
  diffContext,
  diffRemove,
  terminalLine,
  terminalRoot,
  toolCallArgumentGrid,
  toolCallContentSection,
  toolCallLabel,
  toolCallValue,
  toolCallBlock,
  toolCallBody,
  toolCallHeader,
} from "../../styles/session-view.css";

type ToolCallBlockType = Extract<ContentBlock, { tag: "ToolCall" }>;

interface Props {
  block: ToolCallBlockType;
}

type ToolKind = "read" | "write" | "terminal" | "search" | "other";

function classifyTool(toolName: string): ToolKind {
  const name = toolName.toLowerCase();
  if (["read", "read file", "read_file", "readtextfile"].includes(name)) return "read";
  if (["write", "write file", "write_file", "edit", "notebookedit"].includes(name)) return "write";
  if (["bash", "terminal", "run", "create terminal", "create_terminal"].includes(name))
    return "terminal";
  if (["grep", "glob", "search"].includes(name)) return "search";
  return "other";
}

function parseArgs(raw: string): Record<string, string> {
  try {
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") {
      return Object.fromEntries(
        Object.entries(parsed).map(([k, value]) => [
          k,
          typeof value === "string"
            ? formatDisplayText(value)
            : formatDisplayText(JSON.stringify(value, null, 2)),
        ]),
      );
    }
  } catch {
    // ignored
  }
  return {};
}

function firstPath(locations: ToolCallLocation[], args: Record<string, string>): string {
  const path = args.path ?? args.file_path ?? locations[0]?.path ?? "";
  return path ? formatDisplayPath(path) : "";
}

function buildUnifiedDiff(content: Extract<ToolCallContent, { tag: "Diff" }>): string {
  const oldLines = (content.old_text ?? "").split("\n");
  const newLines = content.new_text.split("\n");
  return [
    `--- a/${content.path}`,
    `+++ b/${content.path}`,
    ...oldLines.filter(Boolean).map((line) => `-${line}`),
    ...newLines.filter(Boolean).map((line) => `+${line}`),
  ].join("\n");
}

function changedLineCounts(oldText: string, newText: string): { added: number; removed: number } {
  const oldLines = oldText.split("\n");
  const newLines = newText.split("\n");

  let prefix = 0;
  while (
    prefix < oldLines.length &&
    prefix < newLines.length &&
    oldLines[prefix] === newLines[prefix]
  ) {
    prefix += 1;
  }

  let oldSuffix = oldLines.length - 1;
  let newSuffix = newLines.length - 1;
  while (
    oldSuffix >= prefix &&
    newSuffix >= prefix &&
    oldLines[oldSuffix] === newLines[newSuffix]
  ) {
    oldSuffix -= 1;
    newSuffix -= 1;
  }

  return {
    added: Math.max(0, newSuffix - prefix + 1),
    removed: Math.max(0, oldSuffix - prefix + 1),
  };
}

function diffStats(contents: ToolCallContent[]): string {
  const diff = contents.find((item) => item.tag === "Diff");
  if (!diff) return "";
  const { added, removed } = changedLineCounts(diff.old_text ?? "", diff.new_text);
  return `+${added} -${removed}`;
}

export function collapsedSummary(
  toolName: string,
  args: Record<string, string>,
  contents: ToolCallContent[],
  locations: ToolCallLocation[],
): string {
  const kind = classifyTool(toolName);
  switch (kind) {
    case "read":
      return firstPath(locations, args);
    case "write": {
      const path = firstPath(locations, args);
      const stats = diffStats(contents);
      return stats ? `${path}  ${stats}` : path;
    }
    case "terminal": {
      const terminal = contents.find((item) => item.tag === "Terminal");
      return args.command ?? args.cmd ?? terminal?.terminal_id ?? "";
    }
    case "search":
      return args.pattern ?? args.query ?? args.glob ?? args.include ?? "";
    default:
      return firstPath(locations, args) || args.command || args.pattern || "";
  }
}

function DiffView({ content }: { content: string }) {
  return (
    <ScrollArea style={{ maxHeight: "20rem", maxWidth: "100%" }}>
      <Box style={{ fontFamily: "monospace", fontSize: "var(--font-size-1)", whiteSpace: "pre" }}>
        {content.split("\n").map((line, i) => {
          if (line.startsWith("+") && !line.startsWith("+++")) {
            return (
              <span key={i} className={diffAdd}>
                {line}
              </span>
            );
          }
          if (line.startsWith("-") && !line.startsWith("---")) {
            return (
              <span key={i} className={diffRemove}>
                {line}
              </span>
            );
          }
          return (
            <span key={i} className={diffContext}>
              {line}
            </span>
          );
        })}
      </Box>
    </ScrollArea>
  );
}

function ToolArguments({ args, raw }: { args: Record<string, string>; raw: string }) {
  const entries = Object.entries(args);
  if (entries.length === 0) {
    if (!raw || raw === "{}") return null;
    return (
      <Code size="1" style={{ whiteSpace: "pre-wrap", color: "var(--gray-11)" }}>
        {formatDisplayText(raw)}
      </Code>
    );
  }

  return (
    <Box className={toolCallArgumentGrid}>
      {entries.map(([key, value]) => (
        <Fragment key={key}>
          <Text size="1" className={toolCallLabel}>
            {key}
          </Text>
          <Text size="1" className={toolCallValue}>
            {value}
          </Text>
        </Fragment>
      ))}
    </Box>
  );
}

function ToolLocations({ locations }: { locations: ToolCallLocation[] }) {
  if (locations.length === 0) return null;
  return (
    <Flex direction="column" gap="1">
      {locations.map((location) => (
        <Code key={`${location.path}:${location.line ?? 0}`} size="1">
          {formatDisplayPath(location.path)}
          {location.line ? `:${location.line}` : ""}
        </Code>
      ))}
    </Flex>
  );
}

type AnsiStyle = {
  color?: string;
  fontWeight?: "bold";
  textDecoration?: "underline";
};

const ANSI_COLORS: Record<number, string> = {
  30: "#4b5563",
  31: "#ef4444",
  32: "#22c55e",
  33: "#eab308",
  34: "#60a5fa",
  35: "#c084fc",
  36: "#22d3ee",
  37: "#e5e7eb",
  90: "#6b7280",
  91: "#f87171",
  92: "#4ade80",
  93: "#fde047",
  94: "#93c5fd",
  95: "#d8b4fe",
  96: "#67e8f9",
  97: "#f9fafb",
};

function applyAnsiCodes(style: AnsiStyle, codes: number[]): AnsiStyle {
  let next = { ...style };
  for (const code of codes) {
    if (code === 0) {
      next = {};
    } else if (code === 1) {
      next.fontWeight = "bold";
    } else if (code === 4) {
      next.textDecoration = "underline";
    } else if (code === 22) {
      delete next.fontWeight;
    } else if (code === 24) {
      delete next.textDecoration;
    } else if (code === 39) {
      delete next.color;
    } else if (ANSI_COLORS[code]) {
      next.color = ANSI_COLORS[code];
    }
  }
  return next;
}

function renderAnsiLine(line: string): Array<{ text: string; style: AnsiStyle }> {
  const pattern = new RegExp(`${String.fromCharCode(27)}\\[([0-9;]*)m`, "g");
  const parts: Array<{ text: string; style: AnsiStyle }> = [];
  let style: AnsiStyle = {};
  let lastIndex = 0;

  for (const match of line.matchAll(pattern)) {
    const index = match.index ?? 0;
    if (index > lastIndex) {
      parts.push({ text: line.slice(lastIndex, index), style });
    }
    const rawCodes = match[1] ? match[1].split(";").map((code) => Number(code || "0")) : [0];
    style = applyAnsiCodes(style, rawCodes);
    lastIndex = index + match[0].length;
  }

  if (lastIndex < line.length) {
    parts.push({ text: line.slice(lastIndex), style });
  }

  if (parts.length === 0) {
    parts.push({ text: "", style: {} });
  }

  return parts;
}

function extractMarkdownFence(text: string): { language: string | null; body: string } | null {
  const match = /^```([^\n`]*)\n([\s\S]*?)\n```$/u.exec(text.trim());
  if (!match) return null;
  const language = match[1]?.trim() || null;
  return { language, body: match[2] };
}

function TerminalTranscript({ command, output }: { command?: string; output: string }) {
  const lines = useMemo(() => output.split("\n"), [output]);
  return (
    <Box className={terminalRoot}>
      {command && <Code size="1">$ {command}</Code>}
      <Box>
        {lines.map((line, lineIndex) => (
          <Box key={lineIndex} className={terminalLine}>
            {renderAnsiLine(line).map((part, partIndex) => (
              <span key={partIndex} style={part.style}>
                {part.text || "\u00a0"}
              </span>
            ))}
          </Box>
        ))}
      </Box>
    </Box>
  );
}

function RichTextContent({ text }: { text: string }) {
  const displayText = formatDisplayText(text);
  const fence = extractMarkdownFence(displayText);
  if (
    fence &&
    (fence.language === "console" || fence.language === "terminal" || fence.language === "sh")
  ) {
    return <TerminalTranscript output={fence.body} />;
  }

  return (
    <Box className={toolCallContentSection}>
      <ReactMarkdown>{displayText}</ReactMarkdown>
    </Box>
  );
}

function ToolContents({
  contents,
  args,
}: {
  contents: ToolCallContent[];
  args: Record<string, string>;
}) {
  if (contents.length === 0) return null;
  return (
    <Flex direction="column" gap="2">
      {contents.map((content, index) => {
        switch (content.tag) {
          case "Text":
            return <RichTextContent key={index} text={content.text} />;
          case "Diff":
            return (
              <DiffView
                key={index}
                content={buildUnifiedDiff({ ...content, path: formatDisplayPath(content.path) })}
              />
            );
          case "Terminal":
            return (
              <Flex key={index} direction="column" gap="2">
                {args.command && <Code size="1">$ {args.command}</Code>}
                <Badge color="gray" size="1">
                  terminal {content.terminal_id}
                </Badge>
              </Flex>
            );
        }
      })}
    </Flex>
  );
}

// r[ui.block.tool-call.layout]
// r[ui.block.tool-call.collapsed-default]
// r[ui.block.tool-call.diff]
// r[ui.block.tool-call.terminal]
// r[ui.block.tool-call.search]
export function ToolCallBlock({ block }: Props) {
  const [expanded, setExpanded] = useState(true);

  const args = parseArgs(block.arguments);
  const summary = collapsedSummary(block.tool_name, args, block.content, block.locations);
  const isRunning = block.status.tag === "Running";
  const statusColor =
    block.status.tag === "Success"
      ? "green"
      : block.status.tag === "Failure"
        ? "red"
        : ("gray" as const);

  return (
    <Box className={toolCallBlock}>
      <Flex
        className={toolCallHeader}
        align="center"
        gap="2"
        onClick={() => setExpanded((open) => !open)}
        role="button"
        aria-expanded={expanded}
      >
        {expanded ? (
          <CaretDown size={12} style={{ color: "var(--gray-9)", flexShrink: 0 }} />
        ) : (
          <CaretRight size={12} style={{ color: "var(--gray-9)", flexShrink: 0 }} />
        )}
        <Code size="1">{block.tool_name}</Code>
        {summary && (
          <Text
            size="1"
            style={{
              color: "var(--gray-11)",
              fontFamily: "monospace",
              flex: 1,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {summary}
          </Text>
        )}
        <Box ml="auto" style={{ flexShrink: 0 }}>
          {isRunning ? (
            <Badge color="gray" size="1">
              <Spinner size="1" />
            </Badge>
          ) : (
            <Badge color={statusColor} size="1">
              {block.status.tag === "Success" ? "✓" : "✗"}
            </Badge>
          )}
        </Box>
      </Flex>
      {expanded && (
        <Box className={toolCallBody}>
          <Flex direction="column" gap="2">
            <ToolArguments args={args} raw={block.arguments} />
            <ToolLocations locations={block.locations} />
            <ToolContents contents={block.content} args={args} />
          </Flex>
        </Box>
      )}
    </Box>
  );
}
