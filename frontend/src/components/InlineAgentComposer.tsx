import { useEffect, useRef, useState } from "react";
import { Box, Button, Flex, Text, TextArea } from "@radix-ui/themes";
import { getShipClient } from "../api/client";
import type { Role, SessionStartupState, TaskStatus } from "../generated/ship";
import {
  composerActions,
  composerInput,
  composerInputWrapper,
  composerRoot,
  fileMentionItem,
  fileMentionPopup,
} from "../styles/session-view.css";
import { useWorktreeFiles } from "../hooks/useWorktreeFiles";

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

// r[ui.composer.file-mention]
function getAtMentionQuery(text: string, cursorPos: number): string | null {
  const textBefore = text.slice(0, cursorPos);
  const match = textBefore.match(/@([a-zA-Z0-9/._-]*)$/);
  if (match) return match[1];
  return null;
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
  const [mentionQuery, setMentionQuery] = useState<string | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const status = getStatusCopy(role, agentStateTag, startupState, taskStatus, queuedText);
  const worktreeFiles = useWorktreeFiles(sessionId);

  const filteredFiles =
    mentionQuery !== null
      ? worktreeFiles
          .filter((f) => f.toLowerCase().includes(mentionQuery.toLowerCase()))
          .slice(0, 10)
      : [];

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

  function handleTextChange(event: React.ChangeEvent<HTMLTextAreaElement>) {
    const newText = event.target.value;
    setText(newText);
    const cursorPos = event.target.selectionStart ?? newText.length;
    const query = getAtMentionQuery(newText, cursorPos);
    setMentionQuery(query);
    setSelectedIndex(0);
  }

  function insertMention(file: string) {
    const textarea = textareaRef.current;
    if (!textarea) return;
    const cursorPos = textarea.selectionStart ?? text.length;
    const textBefore = text.slice(0, cursorPos);
    const atIndex = textBefore.lastIndexOf("@");
    if (atIndex === -1) return;
    const newText = text.slice(0, atIndex) + "@" + file + text.slice(cursorPos);
    setText(newText);
    setMentionQuery(null);
    const newCursorPos = atIndex + 1 + file.length;
    requestAnimationFrame(() => {
      textarea.setSelectionRange(newCursorPos, newCursorPos);
      textarea.focus();
    });
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (mentionQuery !== null && filteredFiles.length > 0) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, filteredFiles.length - 1));
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
        return;
      }
      if (event.key === "Enter" && !event.metaKey && !event.ctrlKey) {
        event.preventDefault();
        insertMention(filteredFiles[selectedIndex]);
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        setMentionQuery(null);
        return;
      }
    }

    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      void handleSubmit();
    }
  }

  return (
    <Flex className={composerRoot} direction="column" gap="2">
      <div className={composerInputWrapper}>
        {mentionQuery !== null && filteredFiles.length > 0 && (
          <div className={fileMentionPopup}>
            {filteredFiles.map((file, index) => (
              <div
                key={file}
                className={fileMentionItem}
                data-selected={index === selectedIndex}
                onMouseDown={(e) => {
                  e.preventDefault();
                  insertMention(file);
                }}
              >
                {file}
              </div>
            ))}
          </div>
        )}
        <TextArea
          ref={textareaRef}
          className={composerInput}
          size="2"
          rows={2}
          placeholder={
            role.tag === "Captain" ? "Steer the captain directly…" : "Steer the mate directly…"
          }
          value={text}
          onChange={handleTextChange}
          onKeyDown={handleKeyDown}
          disabled={status.disableInput || loading}
          aria-label={role.tag === "Captain" ? "Captain steer input" : "Mate steer input"}
        />
      </div>
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
