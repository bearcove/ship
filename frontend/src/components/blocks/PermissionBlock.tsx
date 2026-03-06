import { useEffect, useState } from "react";
import { Badge, Box, Button, Code, Flex, Text } from "@radix-ui/themes";
import { CaretDown, CaretRight } from "@phosphor-icons/react";
import type { ContentBlock } from "../../generated/ship";
import { formatDisplayText } from "../../utils/displayPath";
import { permissionCard } from "../../styles/session-view.css";
import {
  firstAllowOption,
  firstRejectOption,
  jsonValueToString,
  optionTone,
  summarizeTarget,
} from "./toolPayload";

type PermissionBlockType = Extract<ContentBlock, { tag: "Permission" }>;

interface Props {
  block: PermissionBlockType;
  onResolve?: (optionId: string) => Promise<void> | void;
}

function RawJson({ value }: { value: string }) {
  return (
    <Box
      style={{
        fontFamily: "monospace",
        fontSize: "var(--font-size-1)",
        background: "var(--gray-a3)",
        borderRadius: "var(--radius-2)",
        padding: "var(--space-2)",
        whiteSpace: "pre-wrap",
      }}
    >
      {value}
    </Box>
  );
}

// r[acp.permissions]
// r[ui.permission.layout]
export function PermissionBlock({ block, onResolve }: Props) {
  const [argsExpanded, setArgsExpanded] = useState(false);
  const [pendingOptionId, setPendingOptionId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const summary =
    summarizeTarget(block.target, block.kind, []) || formatDisplayText(block.description);
  const rawInputText = jsonValueToString(block.raw_input) || formatDisplayText(block.arguments);
  const allowOption = firstAllowOption(block.options);
  const rejectOption = firstRejectOption(block.options);

  async function runAction(optionId: string) {
    if (!onResolve || pendingOptionId) return;
    setPendingOptionId(optionId);
    setError(null);
    try {
      await onResolve(optionId);
    } catch (actionError) {
      setError(actionError instanceof Error ? actionError.message : String(actionError));
    } finally {
      setPendingOptionId(null);
    }
  }

  // r[ui.keys.permission]
  useEffect(() => {
    if (block.resolution) return;
    if (!onResolve) return;

    function handler(event: KeyboardEvent) {
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) {
        return;
      }
      if (event.key === "y" && allowOption) void runAction(allowOption.option_id);
      if (event.key === "n" && rejectOption) void runAction(rejectOption.option_id);
    }

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [allowOption, block.resolution, onResolve, pendingOptionId, rejectOption]);

  // r[ui.permission.resolved]
  if (block.resolution) {
    return (
      <Flex align="center" gap="2">
        <Badge color={block.resolution.tag === "Approved" ? "green" : "red"} size="1">
          {block.resolution.tag === "Approved" ? "✓ Approved" : "✗ Denied"}
        </Badge>
        <Text size="1" color="gray">
          <Code size="1">{formatDisplayText(block.tool_name)}</Code>
          {summary ? ` — ${summary}` : ""}
        </Text>
      </Flex>
    );
  }

  return (
    <Box className={permissionCard}>
      <Flex direction="column" gap="1">
        <Text size="2" weight="medium">
          Permission request
        </Text>
        <Text size="2" style={{ overflowWrap: "anywhere" }}>
          <Code size="1">{formatDisplayText(block.tool_name)}</Code>
          {summary ? ` — ${summary}` : ""}
        </Text>
      </Flex>

      <Flex
        align="center"
        gap="1"
        style={{ cursor: "pointer" }}
        onClick={() => setArgsExpanded((open) => !open)}
      >
        {argsExpanded ? <CaretDown size={12} /> : <CaretRight size={12} />}
        <Text size="1" color="gray">
          Details
        </Text>
      </Flex>
      {argsExpanded && <RawJson value={rawInputText} />}

      {/* r[ui.permission.actions] */}
      {/* r[ui.permission.viewer-mode] */}
      <Flex gap="2" align="center" wrap="wrap">
        {(block.options ?? []).map((option) => {
          const tone = optionTone(option.kind);
          return (
            <Button
              key={option.option_id}
              size="1"
              color={tone.color}
              variant={tone.variant}
              disabled={!onResolve || pendingOptionId !== null}
              loading={pendingOptionId === option.option_id}
              onClick={() => void runAction(option.option_id)}
            >
              {option.label}
            </Button>
          );
        })}
      </Flex>
      {!block.options?.length && (
        <Text size="1" color="gray">
          No permission options available for this request.
        </Text>
      )}
      {error && (
        <Text size="1" color="red">
          {error}
        </Text>
      )}
    </Box>
  );
}
