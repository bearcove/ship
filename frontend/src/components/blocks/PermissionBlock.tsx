import { useEffect, useState } from "react";
import { Badge, Box, Button, Code, Flex, Text, Tooltip } from "@radix-ui/themes";
import { CaretDown, CaretRight } from "@phosphor-icons/react";
import type { ContentBlock, PermissionResolution } from "../../generated/ship";
import { permissionCard } from "../../styles/session-view.css";

type PermissionBlockType = Extract<ContentBlock, { tag: "Permission" }>;

interface Props {
  block: PermissionBlockType;
  onApprove?: () => void;
  onDeny?: () => void;
}

// r[ui.permission.layout]
export function PermissionBlock({ block, onApprove, onDeny }: Props) {
  const [argsExpanded, setArgsExpanded] = useState(false);

  const resolution: PermissionResolution | null = block.resolution;

  // r[ui.keys.permission]
  useEffect(() => {
    if (resolution) return;
    if (!onApprove && !onDeny) return;

    function handler(e: KeyboardEvent) {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key === "y") onApprove?.();
      if (e.key === "n") onDeny?.();
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [resolution, onApprove, onDeny]);

  if (resolution) {
    return (
      <Flex align="center" gap="2">
        <Badge color={resolution.tag === "Approved" ? "green" : "red"} size="1">
          {resolution.tag === "Approved" ? "✓ Approved" : "✗ Denied"}
        </Badge>
        <Text size="1" color="gray">
          <Code size="1">{block.tool_name}</Code> — {block.description}
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
          <Code size="1">{block.tool_name}</Code> — {block.description}
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
          {block.arguments}
        </Box>
      )}

      {/* r[ui.permission.actions] */}
      <Flex gap="2" align="center">
        <Button size="1" color="green" variant="solid" onClick={onApprove}>
          Approve
        </Button>
        <Button size="1" color="red" variant="soft" onClick={onDeny}>
          Deny
        </Button>
        <Tooltip content="Approve all future uses of this tool for the current task">
          <Button size="1" color="green" variant="outline" onClick={onApprove}>
            Approve all {block.tool_name}
          </Button>
        </Tooltip>
        {(onApprove || onDeny) && (
          <Text size="1" color="gray" style={{ marginLeft: "auto" }}>
            y / n
          </Text>
        )}
      </Flex>
    </Box>
  );
}
