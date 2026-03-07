import { useEffect, useState } from "react";
import { Box, Button, Flex, Text, TextArea } from "@radix-ui/themes";
import { getShipClient } from "../api/client";
import type { Role, SessionStartupState, TaskStatus } from "../generated/ship";
import { composerActions, composerInput, composerRoot } from "../styles/session-view.css";

interface Props {
  sessionId: string;
  role: Role;
  agentStateTag: "Working" | "Idle" | "AwaitingPermission" | "ContextExhausted" | "Error";
  startupState: SessionStartupState | null;
  taskStatus: TaskStatus | null;
}

function getStatusCopy(
  role: Role,
  agentStateTag: Props["agentStateTag"],
  startupState: SessionStartupState | null,
  taskStatus: TaskStatus | null,
  queuedText: string | null,
): {
  disableInput: boolean;
  disableSubmit: boolean;
  queueOnSubmit: boolean;
  submitLabel: string;
} {
  if (queuedText) {
    return {
      disableInput: false,
      disableSubmit: false,
      queueOnSubmit: true,
      submitLabel: "Replace queue",
    };
  }

  if (startupState !== null && startupState.tag !== "Ready") {
    if (startupState.tag === "Failed") {
      return {
        disableInput: true,
        disableSubmit: true,
        queueOnSubmit: false,
        submitLabel: "Send",
      };
    }

    if (role.tag === "Captain") {
      const captainBusy = agentStateTag === "Working";
      return {
        disableInput: false,
        disableSubmit: false,
        queueOnSubmit: captainBusy,
        submitLabel: captainBusy ? "Queue" : "Send",
      };
    }

    return {
      disableInput: false,
      disableSubmit: true,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  if (agentStateTag === "AwaitingPermission") {
    return {
      disableInput: true,
      disableSubmit: true,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  if (agentStateTag === "ContextExhausted") {
    return {
      disableInput: true,
      disableSubmit: true,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  if (agentStateTag === "Error") {
    return {
      disableInput: true,
      disableSubmit: true,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  if (role.tag === "Mate" && taskStatus === null) {
    return {
      disableInput: false,
      disableSubmit: true,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  return {
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
