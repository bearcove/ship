import { Fragment, useMemo, useState } from "react";
import { Badge, Box, Code, Flex, ScrollArea, Spinner, Text } from "@radix-ui/themes";
import { CaretDown, CaretRight } from "@phosphor-icons/react";
import ReactMarkdown from "react-markdown";
import type {
  ContentBlock,
  ToolCallContent,
  ToolCallLocation,
  ToolTarget,
} from "../../generated/ship";
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
import { displayTargetPath, diffStats, jsonValueToString, summarizeTarget } from "./toolPayload";

type ToolCallBlockType = Extract<ContentBlock, { tag: "ToolCall" }>;

interface Props {
  block: ToolCallBlockType;
}

function parseLegacyArgs(raw: string): Record<string, string> {
  try {
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") {
      return Object.fromEntries(
        Object.entries(parsed).map(([key, value]) => [
          key,
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

function firstLegacyPath(locations: ToolCallLocation[], args: Record<string, string>): string {
  const path =
    args.path ?? args.file_path ?? locations[0]?.display_path ?? locations[0]?.path ?? "";
  return path ? formatDisplayPath(path) : "";
}

function legacyCollapsedSummary(
  toolName: string,
  args: Record<string, string>,
  contents: ToolCallContent[],
  locations: ToolCallLocation[],
): string {
  const name = toolName.toLowerCase();
  if (["read", "read file", "read_file", "readtextfile"].includes(name)) {
    return firstLegacyPath(locations, args);
  }
  if (["write", "write file", "write_file", "edit", "notebookedit"].includes(name)) {
    const path = firstLegacyPath(locations, args);
    const stats = diffStats(contents);
    return stats ? `${path}  ${stats}` : path;
  }
  if (["bash", "terminal", "run", "create terminal", "create_terminal"].includes(name)) {
    return args.command ?? args.cmd ?? "";
  }
  if (["grep", "glob", "search"].includes(name)) {
    return args.pattern ?? args.query ?? args.glob ?? args.include ?? "";
  }
  return firstLegacyPath(locations, args) || args.command || args.pattern || "";
}

function buildUnifiedDiff(content: Extract<ToolCallContent, { tag: "Diff" }>): string {
  const oldLines = (content.old_text ?? "").split("\n");
  const newLines = content.new_text.split("\n");
  const displayPath = displayTargetPath(content.path, content.display_path);
  return [
    `--- a/${displayPath}`,
    `+++ b/${displayPath}`,
    ...oldLines.filter(Boolean).map((line) => `-${line}`),
    ...newLines.filter(Boolean).map((line) => `+${line}`),
  ].join("\n");
}

function collapsedSummary(block: ToolCallBlockType): string {
  const summary = summarizeTarget(block.target, block.kind, block.content);
  if (summary) return summary;
  return legacyCollapsedSummary(
    block.tool_name,
    parseLegacyArgs(block.arguments),
    block.content,
    block.locations,
  );
}

function DiffView({ content }: { content: string }) {
  return (
    <ScrollArea style={{ maxHeight: "20rem", maxWidth: "100%" }}>
      <Box style={{ fontFamily: "monospace", fontSize: "var(--font-size-1)", whiteSpace: "pre" }}>
        {content.split("\n").map((line, index) => {
          if (line.startsWith("+") && !line.startsWith("+++")) {
            return (
              <span key={index} className={diffAdd}>
                {line}
              </span>
            );
          }
          if (line.startsWith("-") && !line.startsWith("---")) {
            return (
              <span key={index} className={diffRemove}>
                {line}
              </span>
            );
          }
          return (
            <span key={index} className={diffContext}>
              {line}
            </span>
          );
        })}
      </Box>
    </ScrollArea>
  );
}

function StructuredTarget({ target }: { target: ToolTarget | null }) {
  if (!target || target.tag === "None") return null;

  const rows: Array<[string, string]> = [];
  switch (target.tag) {
    case "File":
      rows.push(["path", displayTargetPath(target.path, target.display_path)]);
      if (target.line) rows.push(["line", String(target.line)]);
      break;
    case "Move":
      rows.push(["from", displayTargetPath(target.source_path, target.source_display_path)]);
      rows.push([
        "to",
        displayTargetPath(target.destination_path, target.destination_display_path),
      ]);
      break;
    case "Search":
      if (target.query) rows.push(["query", target.query]);
      if (target.glob) rows.push(["glob", target.glob]);
      if (target.path) {
        rows.push(["path", displayTargetPath(target.path, target.display_path)]);
      }
      break;
    case "Command":
      rows.push(["command", target.command]);
      if (target.cwd) {
        rows.push(["cwd", target.display_cwd ?? formatDisplayPath(target.cwd)]);
      }
      break;
  }

  if (rows.length === 0) return null;
  return (
    <Box className={toolCallArgumentGrid}>
      {rows.map(([label, value]) => (
        <Fragment key={`${label}:${value}`}>
          <Text size="1" className={toolCallLabel}>
            {label}
          </Text>
          <Text size="1" className={toolCallValue}>
            {value}
          </Text>
        </Fragment>
      ))}
    </Box>
  );
}

function RawJson({ value }: { value: string }) {
  return (
    <Code size="1" style={{ whiteSpace: "pre-wrap", color: "var(--gray-11)" }}>
      {value}
    </Code>
  );
}

function ToolArguments({
  target,
  rawInput,
  raw,
}: {
  target: ToolTarget | null;
  rawInput: ToolCallBlockType["raw_input"];
  raw: string;
}) {
  const hasStructuredTarget = target !== null && target.tag !== "None";
  const rawInputText = jsonValueToString(rawInput);
  if (hasStructuredTarget && rawInputText) {
    return (
      <Flex direction="column" gap="2">
        <StructuredTarget target={target} />
        <RawJson value={rawInputText} />
      </Flex>
    );
  }
  if (hasStructuredTarget) return <StructuredTarget target={target} />;
  if (rawInputText) return <RawJson value={rawInputText} />;
  if (!raw || raw === "{}") return null;
  return <RawJson value={formatDisplayText(raw)} />;
}

function ToolLocations({ locations }: { locations: ToolCallLocation[] }) {
  if (locations.length === 0) return null;
  return (
    <Flex direction="column" gap="1">
      {locations.map((location) => (
        <Code key={`${location.path}:${location.line ?? 0}`} size="1">
          {displayTargetPath(location.path, location.display_path)}
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

function TerminalTranscript({
  command,
  output,
  exitLabel,
}: {
  command?: string;
  output: string;
  exitLabel?: string;
}) {
  const lines = useMemo(() => output.split("\n"), [output]);
  return (
    <Box className={terminalRoot}>
      <Flex align="center" justify="between" gap="2">
        {command ? <Code size="1">$ {command}</Code> : <span />}
        {exitLabel && (
          <Badge color={exitLabel === "exit 0" ? "gray" : "red"} size="1">
            {exitLabel}
          </Badge>
        )}
      </Flex>
      <ScrollArea scrollbars="vertical" style={{ maxHeight: "20rem" }}>
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
      </ScrollArea>
    </Box>
  );
}

function terminalExitLabel(
  content: Extract<ToolCallContent, { tag: "Terminal" }>,
): string | undefined {
  if (!content.snapshot?.exit) return undefined;
  if (content.snapshot.exit.exit_code !== null) {
    return `exit ${content.snapshot.exit.exit_code}`;
  }
  if (content.snapshot.exit.signal) {
    return `signal ${content.snapshot.exit.signal}`;
  }
  return undefined;
}

function RichTextContent({ text }: { text: string }) {
  return (
    <Box className={toolCallContentSection}>
      <ReactMarkdown>{formatDisplayText(text)}</ReactMarkdown>
    </Box>
  );
}

function ToolContents({ contents, command }: { contents: ToolCallContent[]; command?: string }) {
  if (contents.length === 0) return null;
  return (
    <Flex direction="column" gap="2">
      {contents.map((content, index) => {
        switch (content.tag) {
          case "Text":
            return <RichTextContent key={index} text={content.text} />;
          case "Diff":
            return <DiffView key={index} content={buildUnifiedDiff(content)} />;
          case "Terminal":
            if (!content.snapshot) {
              return (
                <Badge key={index} color="gray" size="1">
                  terminal {content.terminal_id}
                </Badge>
              );
            }
            return (
              <TerminalTranscript
                key={index}
                command={command}
                output={content.snapshot.output}
                exitLabel={terminalExitLabel(content)}
              />
            );
          case "Raw":
            return <RawJson key={index} value={jsonValueToString(content.data)} />;
        }
      })}
    </Flex>
  );
}

function ToolError({ block }: { block: ToolCallBlockType }) {
  if (!block.error) return null;
  return (
    <Flex
      direction="column"
      gap="2"
      p="2"
      style={{
        borderRadius: "var(--radius-2)",
        background: "var(--red-a3)",
        border: "1px solid var(--red-a5)",
      }}
    >
      <Text size="2" color="red" weight="medium">
        {block.error.message}
      </Text>
      {block.error.details && <RawJson value={jsonValueToString(block.error.details)} />}
    </Flex>
  );
}

function commandFromTarget(target: ToolTarget | null): string | undefined {
  return target?.tag === "Command" ? target.command : undefined;
}

function toolCallStatusLabel(status: ToolCallBlockType["status"]): string {
  switch (status.tag) {
    case "Running":
      return "Running";
    case "Success":
      return "Success";
    case "Failure":
      return "Failed";
  }
}

// r[acp.content-blocks]
// r[acp.terminals]
// r[ui.block.tool-call.layout]
// r[ui.block.tool-call.collapsed-default]
// r[ui.block.tool-call.diff]
// r[ui.block.tool-call.terminal]
// r[ui.block.tool-call.search]
// r[view.no-terminal]
export function ToolCallBlock({ block }: Props) {
  const [expanded, setExpanded] = useState(false);

  const summary = collapsedSummary(block);
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
              minWidth: 0,
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
              <Flex align="center" gap="1">
                <Spinner size="1" />
                <span>{toolCallStatusLabel(block.status)}</span>
              </Flex>
            </Badge>
          ) : (
            <Badge color={statusColor} size="1">
              {block.status.tag === "Success" ? "✓ " : "✗ "}
              {toolCallStatusLabel(block.status)}
            </Badge>
          )}
        </Box>
      </Flex>
      {expanded && (
        <Box className={toolCallBody}>
          <Flex direction="column" gap="2">
            <ToolArguments target={block.target} rawInput={block.raw_input} raw={block.arguments} />
            <ToolLocations locations={block.locations} />
            <ToolContents contents={block.content} command={commandFromTarget(block.target)} />
            {block.raw_output && block.content.length === 0 && !block.error && (
              <RawJson value={jsonValueToString(block.raw_output)} />
            )}
            <ToolError block={block} />
          </Flex>
        </Box>
      )}
    </Box>
  );
}
