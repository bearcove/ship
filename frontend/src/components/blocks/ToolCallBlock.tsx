import { useState } from "react";
import { Badge, Box, Code, Flex, Text } from "@radix-ui/themes";
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
          {block.arguments}
        </Text>
        <Box ml="auto">
          {block.status.tag === "Running" ? (
            <Badge color="gray" size="1">
              running
            </Badge>
          ) : (
            <Badge color={statusColor} size="1">
              {block.status.tag === "Success" ? "✓" : "✗"}
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
