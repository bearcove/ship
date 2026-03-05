import { useState } from "react";
import { Badge, Box, Code, Flex, Text } from "@radix-ui/themes";
import { CaretRight, CaretDown } from "@phosphor-icons/react";
import type { ToolCallBlock as ToolCallBlockType } from "../../types";
import {
  toolCallBlock,
  toolCallHeader,
  toolCallBody,
  diffAdd,
  diffRemove,
  diffContext,
} from "../../styles/session-view.css";

interface Props {
  block: ToolCallBlockType;
}

function DiffView({ content }: { content: string }) {
  return (
    <Box>
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
  );
}

function isLikelyDiff(content: string): boolean {
  return content.includes("--- a/") || content.includes("+++ b/") || content.includes("@@");
}

export function ToolCallBlock({ block }: Props) {
  const [expanded, setExpanded] = useState(false);

  const statusColor =
    block.status === "success" ? "green" : block.status === "failure" ? "red" : "gray";

  const summary = block.filePath
    ? block.diffSummary
      ? `${block.filePath}  ${block.diffSummary}`
      : block.filePath
    : (block.command ?? block.query ?? "");

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
        <Code size="1">{block.toolName}</Code>
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
        <Box ml="auto">
          {block.status === "pending" ? (
            <Badge color="gray" size="1">
              pending
            </Badge>
          ) : (
            <Badge color={statusColor} size="1">
              {block.status === "success" ? "✓" : "✗"}
            </Badge>
          )}
        </Box>
      </Flex>
      {expanded && block.result && (
        <Box className={toolCallBody}>
          {isLikelyDiff(block.result) ? <DiffView content={block.result} /> : block.result}
        </Box>
      )}
    </Box>
  );
}
