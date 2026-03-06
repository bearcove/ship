import { useState } from "react";
import { Button, Flex, Text, TextArea } from "@radix-ui/themes";
import { getShipClient } from "../api/client";
import type { Role, TaskStatus } from "../generated/ship";
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
  taskStatus: TaskStatus | null;
}

function getStatusCopy(
  role: Role,
  agentStateTag: Props["agentStateTag"],
  taskStatus: TaskStatus | null,
): { label: string; hint: string | null; disabled: boolean } {
  if (role.tag === "Mate") {
    if (taskStatus?.tag === "ReviewPending" || taskStatus?.tag === "SteerPending") {
      return {
        label: agentStateTag === "Working" ? "Working" : "Idle",
        hint: "Send direct steer to the mate.",
        disabled: false,
      };
    }

    return {
      label: agentStateTag === "Working" ? "Working" : "Idle",
      hint: "Mate direct steer becomes available when review is pending.",
      disabled: true,
    };
  }

  if (taskStatus === null) {
    return {
      label: agentStateTag === "Working" ? "Working" : "Idle",
      hint: "Captain steering is available once a task is active.",
      disabled: true,
    };
  }

  return {
    label: agentStateTag === "Working" ? "Working" : "Idle",
    hint: "Send direct guidance to the captain.",
    disabled: false,
  };
}

// r[ui.keys.steer-send]
export function InlineAgentComposer({ sessionId, role, agentStateTag, taskStatus }: Props) {
  const [text, setText] = useState("");
  const [loading, setLoading] = useState(false);
  const status = getStatusCopy(role, agentStateTag, taskStatus);

  async function handleSubmit() {
    const value = text.trim();
    if (!value || loading || status.disabled) return;

    setLoading(true);
    try {
      const client = await getShipClient();
      if (role.tag === "Captain") {
        await client.promptCaptain(sessionId, value);
      } else {
        await client.steer(sessionId, value);
      }
      setText("");
    } catch (error) {
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
      <Flex align="center" justify="between" gap="2">
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
      <Flex className={composerActions} align="center" justify="between" gap="2">
        <Text size="1" color="gray">
          Cmd+Enter to send
        </Text>
        <Button
          size="1"
          onClick={() => void handleSubmit()}
          disabled={!text.trim() || status.disabled}
        >
          Send
        </Button>
      </Flex>
    </Flex>
  );
}
