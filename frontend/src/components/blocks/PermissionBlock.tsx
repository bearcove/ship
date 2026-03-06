import { type ReactElement, useEffect, useState } from "react";
import { Badge, Box, Button, Code, Flex, Text, Tooltip } from "@radix-ui/themes";
import type { ContentBlock } from "../../generated/ship";
import { formatDisplayText } from "../../utils/displayPath";
import { permissionCard } from "../../styles/session-view.css";
import {
  firstAllowOption,
  firstRejectOption,
  optionTone,
  permissionOptionLabel,
  permissionOptionTooltip,
  summarizeTarget,
} from "./toolPayload";

type PermissionBlockType = Extract<ContentBlock, { tag: "Permission" }>;

interface Props {
  block: PermissionBlockType;
  onResolve?: (optionId: string) => Promise<void> | void;
}

function ButtonWithOptionalTooltip({
  content,
  children,
}: {
  content: string | undefined;
  children: ReactElement;
}) {
  if (!content) return children;
  return <Tooltip content={content}>{children}</Tooltip>;
}

// r[acp.permissions]
// r[ui.permission.layout]
export function PermissionBlock({ block, onResolve }: Props) {
  const [pendingOptionId, setPendingOptionId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const toolDisplayName = formatDisplayText(block.tool_name);
  const rawSummary =
    summarizeTarget(block.target, block.kind, []) || formatDisplayText(block.description);
  const summary = rawSummary && rawSummary !== toolDisplayName ? rawSummary : null;
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
          <Code size="1">{toolDisplayName}</Code>
          {summary ? ` — ${summary}` : ""}
        </Text>
      </Flex>
    );
  }

  return (
    <Box className={permissionCard}>
      {/* r[ui.permission.actions] */}
      {/* r[ui.permission.viewer-mode] */}
      <Flex align="center" gap="2" wrap="wrap">
        <Text size="2" style={{ overflowWrap: "anywhere", flex: 1 }}>
          <Code size="1">{toolDisplayName}</Code>
          {summary ? ` — ${summary}` : ""}
        </Text>
        <Flex gap="2" align="center" style={{ flexShrink: 0 }}>
          {(block.options ?? []).map((option) => {
            const tone = optionTone(option.kind);
            const label = permissionOptionLabel(option, block.tool_name);
            return (
              <ButtonWithOptionalTooltip
                key={option.option_id}
                content={permissionOptionTooltip(option)}
              >
                <Button
                  size="1"
                  color={tone.color}
                  variant={tone.variant}
                  disabled={!onResolve || pendingOptionId !== null}
                  loading={pendingOptionId === option.option_id}
                  onClick={() => void runAction(option.option_id)}
                >
                  {label}
                </Button>
              </ButtonWithOptionalTooltip>
            );
          })}
        </Flex>
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
