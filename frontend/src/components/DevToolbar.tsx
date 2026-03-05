import { useEffect, useState } from "react";
import { Badge, Box, Button, Flex, Text } from "@radix-ui/themes";
import { useScenario } from "../context/ScenarioContext";
import type { SessionListScenarioKey, SessionScenarioKey } from "../types";

const SESSION_SCENARIOS: { key: SessionScenarioKey; label: string }[] = [
  { key: "happy-path", label: "Happy path" },
  { key: "captain-idle-mate-working", label: "Captain idle / mate working" },
  { key: "mate-awaiting-permission", label: "Mate awaiting permission" },
  { key: "agent-error", label: "Agent error" },
  { key: "context-exhausted", label: "Context exhausted" },
  { key: "steer-pending", label: "Steer pending" },
  { key: "no-active-task", label: "No active task" },
  { key: "autonomous-mode", label: "Autonomous mode" },
];

const LIST_SCENARIOS: { key: SessionListScenarioKey; label: string }[] = [
  { key: "normal", label: "Normal" },
  { key: "empty", label: "Empty" },
  { key: "with-idle-reminders", label: "Idle reminders" },
];

export function DevToolbar() {
  const [open, setOpen] = useState(false);
  const { sessionScenario, setSessionScenario, sessionListScenario, setSessionListScenario } =
    useScenario();

  useEffect(() => {
    function handler(e: KeyboardEvent) {
      if (e.key === "`") setOpen((o) => !o);
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  if (!open) {
    return (
      <Box
        style={{
          position: "fixed",
          bottom: 8,
          right: 8,
          zIndex: 9999,
        }}
      >
        <Button
          size="1"
          variant="outline"
          color="gray"
          onClick={() => setOpen(true)}
          style={{ opacity: 0.6, fontSize: "10px" }}
        >
          DEV
        </Button>
      </Box>
    );
  }

  return (
    <Box
      style={{
        position: "fixed",
        bottom: 0,
        left: 0,
        right: 0,
        zIndex: 9999,
        background: "var(--color-panel-solid)",
        borderTop: "1px solid var(--gray-a6)",
        padding: "var(--space-3)",
      }}
    >
      <Flex align="start" gap="6" wrap="wrap">
        <Flex direction="column" gap="2">
          <Flex align="center" gap="2">
            <Text size="1" weight="bold" color="gray">
              SESSION SCENARIO
            </Text>
            <Badge size="1" color="gray" variant="outline">
              ` to close
            </Badge>
          </Flex>
          <Flex gap="1" wrap="wrap">
            {SESSION_SCENARIOS.map(({ key, label }) => (
              <Button
                key={key}
                size="1"
                variant={sessionScenario === key ? "solid" : "outline"}
                color={sessionScenario === key ? "iris" : "gray"}
                onClick={() => setSessionScenario(key)}
              >
                {label}
              </Button>
            ))}
          </Flex>
        </Flex>

        <Flex direction="column" gap="2">
          <Text size="1" weight="bold" color="gray">
            SESSION LIST
          </Text>
          <Flex gap="1">
            {LIST_SCENARIOS.map(({ key, label }) => (
              <Button
                key={key}
                size="1"
                variant={sessionListScenario === key ? "solid" : "outline"}
                color={sessionListScenario === key ? "iris" : "gray"}
                onClick={() => setSessionListScenario(key)}
              >
                {label}
              </Button>
            ))}
          </Flex>
        </Flex>

        <Box style={{ marginLeft: "auto" }}>
          <Button size="1" variant="ghost" color="gray" onClick={() => setOpen(false)}>
            Close
          </Button>
        </Box>
      </Flex>
    </Box>
  );
}
