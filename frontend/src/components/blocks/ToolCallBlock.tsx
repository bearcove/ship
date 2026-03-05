import { useState } from "react";
import { Badge, Box, Code, Flex, ScrollArea, Spinner, Text } from "@radix-ui/themes";
import { CaretRight, CaretDown } from "@phosphor-icons/react";
import type { ContentBlock } from "../../generated/ship";
import {
  toolCallBlock,
  toolCallHeader,
  toolCallBody,
  diffAdd,
  diffRemove,
  diffContext,
} from "../../styles/session-view.css";

type ToolCallBlockType = Extract<ContentBlock, { tag: "ToolCall" }>;

interface Props {
  block: ToolCallBlockType;
}

// ─── Tool kind classification ─────────────────────────────────────────────────

type ToolKind = "read" | "write" | "terminal" | "search" | "other";

function classifyTool(toolName: string): ToolKind {
  const name = toolName.toLowerCase();
  if (name === "read") return "read";
  if (name === "write" || name === "edit" || name === "notebookedit") return "write";
  if (name === "bash" || name === "terminal" || name === "run") return "terminal";
  if (name === "grep" || name === "glob" || name === "search") return "search";
  return "other";
}

function parseArgs(raw: string): Record<string, string> {
  try {
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") {
      return Object.fromEntries(Object.entries(parsed).map(([k, v]) => [k, String(v)]));
    }
  } catch {
    // not JSON — return empty
  }
  return {};
}

// ─── Diff utilities ───────────────────────────────────────────────────────────

function isLikelyDiff(content: string): boolean {
  return content.includes("--- a/") || content.includes("+++ b/") || content.includes("@@");
}

function diffSummary(content: string): string {
  let added = 0;
  let removed = 0;
  for (const line of content.split("\n")) {
    if (line.startsWith("+") && !line.startsWith("+++")) added++;
    else if (line.startsWith("-") && !line.startsWith("---")) removed++;
  }
  if (added === 0 && removed === 0) return "";
  return `+${added} -${removed}`;
}

// ─── Collapsed summary ────────────────────────────────────────────────────────

export function collapsedSummary(
  toolName: string,
  args: Record<string, string>,
  result: string | null,
): string {
  const kind = classifyTool(toolName);
  switch (kind) {
    case "read":
      return args.path ?? args.file_path ?? "";
    case "write": {
      const path = args.path ?? args.file_path ?? args.old_string?.slice(0, 30) ?? "";
      if (result && isLikelyDiff(result)) {
        const summary = diffSummary(result);
        return summary ? `${path}  ${summary}` : path;
      }
      return path;
    }
    case "terminal":
      return args.command ?? args.cmd ?? "";
    case "search":
      return args.pattern ?? args.query ?? args.glob ?? args.include ?? "";
    default:
      return args.path ?? args.command ?? args.pattern ?? "";
  }
}

// ─── Expanded: diff view ──────────────────────────────────────────────────────

function DiffView({ content }: { content: string }) {
  return (
    <ScrollArea style={{ maxHeight: "20rem" }}>
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

// ─── Expanded: terminal view ──────────────────────────────────────────────────

function parseExitCode(result: string): number | null {
  // Look for patterns like "Exit code: 1", "exit code 2", "exited with 1"
  const m = result.match(/exit(?:ed)?(?: code)?[:\s]+(\d+)/i);
  if (m) return parseInt(m[1], 10);
  return null;
}

function TerminalView({ result, args }: { result: string; args: Record<string, string> }) {
  const exitCode = parseExitCode(result);
  return (
    <Flex direction="column" gap="2">
      {args.command && (
        <Code size="1" style={{ color: "var(--gray-11)" }}>
          $ {args.command}
        </Code>
      )}
      <ScrollArea style={{ maxHeight: "20rem" }}>
        <Box
          style={{
            fontFamily: "monospace",
            fontSize: "var(--font-size-1)",
            whiteSpace: "pre-wrap",
            color: "var(--gray-12)",
          }}
        >
          {result}
        </Box>
      </ScrollArea>
      {exitCode !== null && exitCode !== 0 && (
        <Box>
          <Badge color="red" size="1">
            exit {exitCode}
          </Badge>
        </Box>
      )}
    </Flex>
  );
}

// ─── Expanded: search results ─────────────────────────────────────────────────

function SearchResultsView({ result }: { result: string }) {
  const lines = result.split("\n").filter(Boolean);
  // Try to detect "file:line:content" format
  const snippets = lines.map((line) => {
    const m = line.match(/^([^:]+):(\d+):(.*)/);
    if (m) return { file: m[1], line: m[2], content: m[3] };
    return { file: null, line: null, content: line };
  });

  return (
    <ScrollArea style={{ maxHeight: "20rem" }}>
      <Flex direction="column" gap="1">
        {snippets.map((s, i) =>
          s.file ? (
            <Flex key={i} gap="2" align="baseline">
              <Code size="1" style={{ color: "var(--blue-11)", flexShrink: 0 }}>
                {s.file}:{s.line}
              </Code>
              <Text
                size="1"
                style={{
                  fontFamily: "monospace",
                  color: "var(--gray-11)",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {s.content.trim()}
              </Text>
            </Flex>
          ) : (
            <Code key={i} size="1" style={{ color: "var(--gray-11)" }}>
              {s.content}
            </Code>
          ),
        )}
      </Flex>
    </ScrollArea>
  );
}

// ─── Main component ───────────────────────────────────────────────────────────

// r[ui.block.tool-call.layout]
// r[ui.block.tool-call.collapsed-default]
// r[ui.block.tool-call.diff]
// r[ui.block.tool-call.terminal]
// r[ui.block.tool-call.search]
export function ToolCallBlock({ block }: Props) {
  const [expanded, setExpanded] = useState(false);

  const args = parseArgs(block.arguments);
  const kind = classifyTool(block.tool_name);
  const summary = collapsedSummary(block.tool_name, args, block.result);

  const isRunning = block.status.tag === "Running";
  const statusColor =
    block.status.tag === "Success"
      ? "green"
      : block.status.tag === "Failure"
        ? "red"
        : ("gray" as const);

  function renderExpandedResult() {
    if (!block.result) return null;
    if (kind === "terminal") {
      return <TerminalView result={block.result} args={args} />;
    }
    if (kind === "search") {
      return <SearchResultsView result={block.result} />;
    }
    if (isLikelyDiff(block.result)) {
      return <DiffView content={block.result} />;
    }
    return (
      <ScrollArea style={{ maxHeight: "20rem" }}>
        <Box
          style={{
            fontFamily: "monospace",
            fontSize: "var(--font-size-1)",
            whiteSpace: "pre-wrap",
          }}
        >
          {block.result}
        </Box>
      </ScrollArea>
    );
  }

  return (
    <Box className={toolCallBlock}>
      <Flex
        className={toolCallHeader}
        align="center"
        gap="2"
        onClick={() => setExpanded((e) => !e)}
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
            <Code size="1" style={{ whiteSpace: "pre-wrap", color: "var(--gray-11)" }}>
              {block.arguments}
            </Code>
            {renderExpandedResult()}
          </Flex>
        </Box>
      )}
    </Box>
  );
}
