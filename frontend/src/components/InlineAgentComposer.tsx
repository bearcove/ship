import { useEffect, useState } from "react";
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
  queuedText: string | null,
): {
  label: string;
  hint: string | null;
  disableInput: boolean;
  disableSubmit: boolean;
  queueOnSubmit: boolean;
  submitLabel: string;
} {
  if (queuedText) {
    return {
      label: "Queued",
      hint: "Your message will send as soon as session startup finishes.",
      disableInput: false,
      disableSubmit: false,
      queueOnSubmit: true,
      submitLabel: "Replace queue",
    };
  }

  if (startupState !== null && startupState.tag !== "Ready") {
    if (startupState.tag === "Failed") {
      return {
        label: "Failed",
        hint: startupState.message,
        disableInput: true,
        disableSubmit: true,
        queueOnSubmit: false,
        submitLabel: "Send",
      };
    }

    if (role.tag === "Captain") {
      const captainBusy = agentStateTag === "Working";
      return {
        label: captainBusy ? "Starting" : stateLabel(agentStateTag),
        hint: captainBusy
          ? "You can type now and queue a captain note while the greeting finishes."
          : "Captain is ready. Mate startup can continue in the background.",
        disableInput: false,
        disableSubmit: false,
        queueOnSubmit: captainBusy,
        submitLabel: captainBusy ? "Queue" : "Send",
      };
    }

    return {
      label: "Starting",
      hint: "You can draft mate steer now. Sending unlocks after startup and task setup.",
      disableInput: false,
      disableSubmit: true,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  if (agentStateTag === "AwaitingPermission") {
    return {
      label: stateLabel(agentStateTag),
      hint: "Approve the pending permission request before sending more guidance.",
      disableInput: true,
      disableSubmit: true,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  if (agentStateTag === "ContextExhausted") {
    return {
      label: stateLabel(agentStateTag),
      hint: "Rotate or retry the agent before sending more guidance.",
      disableInput: true,
      disableSubmit: true,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  if (agentStateTag === "Error") {
    return {
      label: stateLabel(agentStateTag),
      hint: "Retry the agent before sending more guidance.",
      disableInput: true,
      disableSubmit: true,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  if (role.tag === "Mate") {
    if (taskStatus === null) {
      return {
        label: "No active task",
        hint: "Assign a task before steering the mate directly.",
        disableInput: false,
        disableSubmit: true,
        queueOnSubmit: false,
        submitLabel: "Send",
      };
    }

    if (taskStatus.tag === "SteerPending") {
      return {
        label: "Captain steer pending",
        hint: "Send your own steer here to override the captain's pending draft.",
        disableInput: false,
        disableSubmit: false,
        queueOnSubmit: false,
        submitLabel: "Send",
      };
    }

    if (taskStatus.tag === "ReviewPending") {
      return {
        label: "Awaiting review",
        hint: "Need to bypass the captain? Send human steer directly to the mate.",
        disableInput: false,
        disableSubmit: false,
        queueOnSubmit: false,
        submitLabel: "Send",
      };
    }

    return {
      label: stateLabel(agentStateTag),
      hint: "Human steer can redirect the mate at any time during the task.",
      disableInput: false,
      disableSubmit: false,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  if (taskStatus === null) {
    return {
      label: stateLabel(agentStateTag),
      hint: "Talk to the captain directly. Assign a task when you want work to start.",
      disableInput: false,
      disableSubmit: false,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  if (taskStatus.tag === "SteerPending") {
    return {
      label: "Steer pending",
      hint: "Ask the captain to revise or clarify the pending steer before you send it.",
      disableInput: false,
      disableSubmit: false,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  if (taskStatus.tag === "ReviewPending") {
    return {
      label: "Review pending",
      hint: "Ask the captain to review the mate's latest work or draft the next steer.",
      disableInput: false,
      disableSubmit: false,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  return {
    label: stateLabel(agentStateTag),
    hint: "Send direct guidance to the captain.",
    disableInput: false,
    disableSubmit: false,
    queueOnSubmit: false,
    submitLabel: "Send",
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
  const [queuedText, setQueuedText] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const status = getStatusCopy(role, agentStateTag, startupState, taskStatus, queuedText);

  async function sendNow(value: string) {
    setLoading(true);
    setError(null);
    try {
      const client = await getShipClient();
      if (role.tag === "Captain") {
        await client.promptCaptain(sessionId, value);
      } else {
        await client.steer(sessionId, value);
      }
      setQueuedText(null);
      return true;
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
      console.error("[ship/session] failed to send inline guidance", {
        sessionId,
        role: role.tag,
        error,
      });
      return false;
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (role.tag !== "Captain" || !queuedText || loading) {
      return;
    }

    if (startupState?.tag === "Failed") {
      return;
    }

    if (agentStateTag === "Working") {
      return;
    }

    void (async () => {
      await sendNow(queuedText);
    })();
  }, [agentStateTag, loading, queuedText, role.tag, startupState?.tag]);

  async function handleSubmit() {
    const value = text.trim();
    if (!value || loading || status.disableSubmit) return;

    if (status.queueOnSubmit) {
      setQueuedText(value);
      setText("");
      setError(null);
      return;
    }

    if (await sendNow(value)) {
      setText("");
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
        disabled={status.disableInput || loading}
        aria-label={role.tag === "Captain" ? "Captain steer input" : "Mate steer input"}
      />
      <Flex className={composerActions} align="center" justify="end" gap="2">
        <Button
          size="1"
          onClick={() => void handleSubmit()}
          disabled={!text.trim() || status.disableSubmit}
          loading={loading}
        >
          {status.submitLabel}{" "}
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
