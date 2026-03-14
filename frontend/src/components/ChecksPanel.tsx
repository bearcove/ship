import { useState } from "react";
import { Badge, Box, Card, Flex, IconButton, ScrollArea, Spinner, Text } from "@radix-ui/themes";
import { Check, CaretDown, CaretRight, X, XCircle } from "@phosphor-icons/react";
import type { ChecksState } from "../state/sessionReducer";
import { checksCard, checksHookOutput, checksHookRow } from "../styles/session-view.css";
import type { HookCheckResult } from "../generated/ship";

interface Props {
  checks: ChecksState;
  onDismiss: () => void;
}

function HookRow({
  hook,
  isRunning,
}: {
  hook: HookCheckResult | { name: string; passed?: undefined; output?: undefined };
  isRunning: boolean;
}) {
  const [expanded, setExpanded] = useState(false);

  const isFinished = hook.passed !== undefined;
  const hasFailed = isFinished && !hook.passed;
  const hasOutput = hasFailed && !!hook.output;

  return (
    <Box className={checksHookRow}>
      <Flex direction="column" gap="1" style={{ flex: 1, minWidth: 0 }}>
        <Flex align="center" gap="2">
          {/* Status icon */}
          <Flex align="center" justify="center" style={{ width: 16, height: 16, flexShrink: 0 }}>
            {!isFinished && isRunning && <Spinner size="1" />}
            {!isFinished && !isRunning && (
              <Box
                style={{
                  width: 8,
                  height: 8,
                  borderRadius: "50%",
                  background: "var(--gray-7)",
                }}
              />
            )}
            {isFinished && hook.passed && (
              <Check size={14} color="var(--green-9)" weight="bold" />
            )}
            {isFinished && !hook.passed && (
              <XCircle size={14} color="var(--red-9)" weight="bold" />
            )}
          </Flex>

          {/* Hook name */}
          <Text
            size="2"
            style={{
              flex: 1,
              minWidth: 0,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              color: hasFailed ? "var(--red-11)" : "var(--gray-12)",
            }}
          >
            {hook.name}
          </Text>

          {/* Expand/collapse toggle for failures with output */}
          {hasOutput && (
            <IconButton
              size="1"
              variant="ghost"
              color="gray"
              onClick={() => setExpanded((v) => !v)}
              aria-label={expanded ? "Hide output" : "Show output"}
            >
              {expanded ? <CaretDown size={12} /> : <CaretRight size={12} />}
            </IconButton>
          )}
        </Flex>

        {/* Expanded failure output */}
        {hasOutput && expanded && (
          <ScrollArea>
            <Box className={checksHookOutput}>{hook.output}</Box>
          </ScrollArea>
        )}
      </Flex>
    </Box>
  );
}

export function ChecksPanel({ checks, onDismiss }: Props) {
  const { context, hooks, status, results } = checks;



  // Build rows: if finished, use results; otherwise use hook names as placeholders
  const rows: Array<HookCheckResult | { name: string }> =
    results.length > 0
      ? results
      : hooks.map((name) => ({ name }));

  // Label for the context
  const contextLabel = context === "post-commit"
    ? "post-commit"
    : context === "pre-merge"
      ? "pre-merge"
      : context;

  const statusLabel =
    status === "running"
      ? "Running…"
      : status === "passed"
        ? "All checks passed"
        : "Checks failed";

  const statusColor =
    status === "running"
      ? ("gray" as const)
      : status === "passed"
        ? ("green" as const)
        : ("red" as const);

  return (
    <Card
      className={checksCard}
      size="2"
      style={
        status === "passed"
          ? { background: "var(--green-a2)", borderColor: "var(--green-a5)" }
          : status === "failed"
            ? { background: "var(--red-a2)", borderColor: "var(--red-a5)" }
            : undefined
      }
    >
      <Flex direction="column" gap="2">
        {/* Header row */}
        <Flex align="center" justify="between" gap="2">
          <Flex align="center" gap="2">
            <Text size="2" weight="medium" color={statusColor}>
              {statusLabel}
            </Text>
            <Badge size="1" variant="soft" color="gray">
              {contextLabel}
            </Badge>
          </Flex>
          <IconButton
            size="1"
            variant="ghost"
            color="gray"
            onClick={onDismiss}
            aria-label="Dismiss checks panel"
          >
            <X size={14} />
          </IconButton>
        </Flex>

        {/* Hook rows */}
        {rows.length > 0 && (
          <Flex direction="column">
            {rows.map((row, i) => (
              <HookRow
                key={row.name + i}
                hook={row as HookCheckResult | { name: string; passed?: undefined; output?: undefined }}
                isRunning={status === "running"}
              />
            ))}
          </Flex>
        )}
      </Flex>
    </Card>
  );
}
