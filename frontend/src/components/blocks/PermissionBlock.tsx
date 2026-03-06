import { useEffect, useState } from "react";
import { Badge, Box, Button, Code, Flex, Text, Tooltip } from "@radix-ui/themes";
import { CaretDown, CaretRight } from "@phosphor-icons/react";
import type { ContentBlock, PermissionResolution } from "../../generated/ship";
import { formatDisplayText } from "../../utils/displayPath";
import { permissionCard } from "../../styles/session-view.css";

type PermissionBlockType = Extract<ContentBlock, { tag: "Permission" }>;

interface Props {
  block: PermissionBlockType;
  onApprove?: () => Promise<void> | void;
  onDeny?: () => Promise<void> | void;
}

// r[ui.permission.layout]
export function PermissionBlock({ block, onApprove, onDeny }: Props) {
  const [argsExpanded, setArgsExpanded] = useState(false);
  const [pendingAction, setPendingAction] = useState<"approve" | "deny" | null>(null);
  const [error, setError] = useState<string | null>(null);

  const resolution: PermissionResolution | null = block.resolution;
  const displayTool = formatDisplayText(block.tool_name);
  const displayDescription = formatDisplayText(block.description);
  const displayArguments = formatDisplayText(block.arguments);

  async function runAction(kind: "approve" | "deny") {
    const action = kind === "approve" ? onApprove : onDeny;
    if (!action || pendingAction) return;
    setPendingAction(kind);
    setError(null);
    try {
      await action();
    } catch (actionError) {
      setError(actionError instanceof Error ? actionError.message : String(actionError));
    } finally {
      setPendingAction(null);
    }
  }

  // r[ui.keys.permission]
  useEffect(() => {
    if (resolution) return;
    if (!onApprove && !onDeny) return;

    function handler(e: KeyboardEvent) {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key === "y") void runAction("approve");
      if (e.key === "n") void runAction("deny");
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [resolution, onApprove, onDeny, pendingAction]);

  // r[ui.permission.resolved]
  if (resolution) {
    return (
      <Flex align="center" gap="2">
        <Badge color={resolution.tag === "Approved" ? "green" : "red"} size="1">
          {resolution.tag === "Approved" ? "✓ Approved" : "✗ Denied"}
        </Badge>
        <Text size="1" color="gray">
          <Code size="1">{displayTool}</Code> — {displayDescription}
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
        <Text size="2">
          <Code size="1">{displayTool}</Code> — {displayDescription}
        </Text>
      </Flex>

      <Flex
        align="center"
        gap="1"
        style={{ cursor: "pointer" }}
        onClick={() => setArgsExpanded((e) => !e)}
      >
        {argsExpanded ? <CaretDown size={12} /> : <CaretRight size={12} />}
        <Text size="1" color="gray">
          Arguments
        </Text>
      </Flex>
      {argsExpanded && (
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
          {displayArguments}
        </Box>
      )}

      {/* r[ui.permission.actions] */}
      {/* r[ui.permission.viewer-mode] */}
      <Flex gap="2" align="center">
        <Button
          size="1"
          color="green"
          variant="solid"
          disabled={!onApprove || pendingAction !== null}
          loading={pendingAction === "approve"}
          onClick={() => void runAction("approve")}
        >
          Approve
        </Button>
        <Button
          size="1"
          color="red"
          variant="soft"
          disabled={!onDeny || pendingAction !== null}
          loading={pendingAction === "deny"}
          onClick={() => void runAction("deny")}
        >
          Deny
        </Button>
        <Tooltip content="Approve all future uses of this tool for the current task">
          <Button
            size="1"
            color="green"
            variant="outline"
            disabled={!onApprove || pendingAction !== null}
            loading={pendingAction === "approve"}
            onClick={() => void runAction("approve")}
          >
            Approve all {displayTool}
          </Button>
        </Tooltip>
      </Flex>
      {error && (
        <Text size="1" color="red">
          {error}
        </Text>
      )}
    </Box>
  );
}
