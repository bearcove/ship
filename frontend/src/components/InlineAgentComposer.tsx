import { useState } from "react";
import { Box, Button, Flex, Text, TextArea } from "@radix-ui/themes";
import { getShipClient } from "../api/client";
import type { Role, SessionStartupState, TaskStatus } from "../generated/ship";
import {
  composerActions,
  composerHint,
  composerInput,
  composerRoot,
  composerStatus,
} from "../styles/session-view.css";

interface Props {
  sessionId: string;
  role: Role;
  agentStateTag: "Working" | "Idle" | "AwaitingPermission" | "ContextExhausted" | "Error";
  startupState: SessionStartupState | null;
  taskStatus: TaskStatus | null;
}

function stateLabel(agentStateTag: Props["agentStateTag"]): string {
  switch (agentStateTag) {
    case "Working":
      return "Working";
    case "Idle":
      return "Ready";
    case "AwaitingPermission":
      return "Needs permission";
    case "ContextExhausted":
      return "Context exhausted";
    case "Error":
      return "Error";
  }
}

function getStatusCopy(
  role: Role,
  agentStateTag: Props["agentStateTag"],
  startupState: SessionStartupState | null,
  taskStatus: TaskStatus | null,
): { label: string; hint: string | null; disabled: boolean } {
  if (startupState !== null && startupState.tag !== "Ready") {
    return {
      label: startupState.tag === "Failed" ? "Failed" : "Starting",
      hint:
        startupState.tag === "Failed"
          ? startupState.message
          : "Session startup is still in progress.",
      disabled: true,
    };
  }

  if (agentStateTag === "AwaitingPermission") {
    return {
      label: stateLabel(agentStateTag),
      hint: "Approve the pending permission request before sending more guidance.",
      disabled: true,
    };
  }

  if (agentStateTag === "ContextExhausted") {
    return {
      label: stateLabel(agentStateTag),
      hint: "Rotate or retry the agent before sending more guidance.",
      disabled: true,
    };
  }

  if (agentStateTag === "Error") {
    return {
      label: stateLabel(agentStateTag),
      hint: "Retry the agent before sending more guidance.",
      disabled: true,
    };
  }

  if (role.tag === "Mate") {
    if (taskStatus === null) {
      return {
        label: "No active task",
        hint: "Assign a task before steering the mate directly.",
        disabled: true,
      };
    }

    if (taskStatus.tag === "SteerPending") {
      return {
        label: "Captain steer pending",
        hint: "Send your own steer here to override the captain's pending draft.",
        disabled: false,
      };
    }

    if (taskStatus.tag === "ReviewPending") {
      return {
        label: "Awaiting review",
        hint: "Need to bypass the captain? Send human steer directly to the mate.",
        disabled: false,
      };
    }

    return {
      label: stateLabel(agentStateTag),
      hint: "Human steer can redirect the mate at any time during the task.",
      disabled: false,
    };
  }

  if (taskStatus === null) {
    return {
      label: stateLabel(agentStateTag),
      hint: "Talk to the captain directly. Assign a task when you want work to start.",
      disabled: false,
    };
  }

  if (taskStatus.tag === "SteerPending") {
    return {
      label: "Steer pending",
      hint: "Ask the captain to revise or clarify the pending steer before you send it.",
      disabled: false,
    };
  }

  if (taskStatus.tag === "ReviewPending") {
    return {
      label: "Review pending",
      hint: "Ask the captain to review the mate's latest work or draft the next steer.",
      disabled: false,
    };
  }

  return {
    label: stateLabel(agentStateTag),
    hint: "Send direct guidance to the captain.",
    disabled: false,
  };
}

// r[ui.keys.steer-send]
export function InlineAgentComposer({
  sessionId,
  role,
  agentStateTag,
  startupState,
  taskStatus,
}: Props) {
  const [text, setText] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const status = getStatusCopy(role, agentStateTag, startupState, taskStatus);

  async function handleSubmit() {
    const value = text.trim();
    if (!value || loading || status.disabled) return;

    setLoading(true);
    setError(null);
    try {
      const client = await getShipClient();
      if (role.tag === "Captain") {
        await client.promptCaptain(sessionId, value);
      } else {
        await client.steer(sessionId, value);
      }
      setText("");
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
      console.error("[ship/session] failed to send inline guidance", {
        sessionId,
        role: role.tag,
        error,
      });
    } finally {
      setLoading(false);
    }
  }

  return (
    <Flex className={composerRoot} direction="column" gap="2">
      <Flex direction="column" gap="1">
        <Text className={composerStatus} size="1" color="gray">
          {status.label}
        </Text>
        {status.hint && (
          <Text className={composerHint} size="1" color="gray">
            {status.hint}
          </Text>
        )}
      </Flex>
      <TextArea
        className={composerInput}
        size="2"
        rows={2}
        placeholder={
          role.tag === "Captain" ? "Steer the captain directly…" : "Steer the mate directly…"
        }
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
            event.preventDefault();
            void handleSubmit();
          }
        }}
        disabled={status.disabled || loading}
        aria-label={role.tag === "Captain" ? "Captain steer input" : "Mate steer input"}
      />
      <Flex className={composerActions} align="center" justify="end" gap="2">
        <Button
          size="1"
          onClick={() => void handleSubmit()}
          disabled={!text.trim() || status.disabled}
          loading={loading}
        >
          Send{" "}
          <Box asChild style={{ opacity: 0.65, fontSize: "11px", fontFamily: "monospace" }}>
            <kbd>⌘↵</kbd>
          </Box>
        </Button>
      </Flex>
      {error && (
        <Text size="1" color="red">
          {error}
        </Text>
      )}
    </Flex>
  );
}
