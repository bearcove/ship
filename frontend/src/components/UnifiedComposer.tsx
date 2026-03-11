import { useEffect, useRef, useState } from "react";
import { Box, Button, Flex, Text, TextArea } from "@radix-ui/themes";
import { PaperclipIcon, Robot, Warning } from "@phosphor-icons/react";
import { getShipClient } from "../api/client";
import type {
  AgentSnapshot,
  PromptContentPart,
  SessionStartupState,
  TaskStatus,
} from "../generated/ship";
import {
  agentStateChip,
  attachedImageRemove,
  attachedImageThumb,
  attachedImageThumbList,
  attachedImageThumbWrapper,
  composerActions,
  composerActivityDot,
  composerInput,
  composerInputWrapper,
  composerRoot,
  fileMentionItem,
  fileMentionPopup,
  pageDropOverlay,
} from "../styles/session-view.css";
import { useWorktreeFiles } from "../hooks/useWorktreeFiles";
import { useDocumentDrop } from "../hooks/useDocumentDrop";

interface AttachedImage {
  id: string;
  mimeType: string;
  data: Uint8Array;
  objectUrl: string;
  name: string;
}

interface Props {
  sessionId: string;
  captain: AgentSnapshot | null;
  mate: AgentSnapshot | null;
  startupState: SessionStartupState | null;
  taskStatus: TaskStatus | null;
}

function parseTarget(text: string): { target: "captain" | "mate"; content: string } {
  const match = text.match(/^@mate\s*/i);
  if (match) return { target: "mate", content: text.slice(match[0].length) };
  return { target: "captain", content: text };
}

function AgentStateChips({
  captain,
  mate,
}: {
  captain: AgentSnapshot | null;
  mate: AgentSnapshot | null;
}) {
  const chips: React.ReactNode[] = [];

  for (const [label, agent] of [
    ["Captain", captain],
    ["Mate", mate],
  ] as [string, AgentSnapshot | null][]) {
    if (!agent) continue;
    const state = agent.state;
    if (state.tag === "Error") {
      chips.push(
        <span key={label} className={agentStateChip} data-tone="error">
          <Warning size={10} />
          {label}: {state.message.slice(0, 40)}
        </span>,
      );
    } else if (state.tag === "ContextExhausted") {
      chips.push(
        <span key={label} className={agentStateChip} data-tone="warn">
          <Warning size={10} />
          {label}: context exhausted
        </span>,
      );
    } else if (agent.context_remaining_percent !== null && agent.context_remaining_percent < 20) {
      chips.push(
        <span key={label} className={agentStateChip} data-tone="warn">
          {label}: {Math.round(agent.context_remaining_percent)}% ctx
        </span>,
      );
    }
  }

  if (chips.length === 0) return null;
  return (
    <Flex gap="2" wrap="wrap" style={{ flexShrink: 0 }}>
      {chips}
    </Flex>
  );
}

const ACTIVE_TASK_STATUS_TAGS = new Set(["Assigned", "Working", "ReviewPending", "SteerPending"]);

// r[ui.keys.steer-send]
// r[ui.composer.image-attach]
// r[view.agent-panel.activity]
export function UnifiedComposer({ sessionId, captain, mate, startupState, taskStatus }: Props) {
  const [text, setText] = useState("");
  const [queuedText, setQueuedText] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mentionQuery, setMentionQuery] = useState<string | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [attachedImages, setAttachedImages] = useState<AttachedImage[]>([]);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const worktreeFiles = useWorktreeFiles(sessionId);
  const isDragOver = useDocumentDrop(addImageFiles);

  const { target } = parseTarget(text);
  const activeAgent = target === "captain" ? captain : mate;
  const captainStateTag = captain?.state.tag ?? "Idle";
  const mateStateTag = mate?.state.tag ?? "Idle";
  const activeStateTag = target === "captain" ? captainStateTag : mateStateTag;

  const startupReady = startupState === null || startupState.tag === "Ready";
  const startupFailed = startupState?.tag === "Failed";
  const agentWorking = activeStateTag === "Working";
  const agentCantSend =
    activeStateTag === "ContextExhausted" || activeStateTag === "Error" || startupFailed;

  const queueOnSubmit = agentWorking && !queuedText;
  const submitLabel =
    target === "mate"
      ? "Steer mate"
      : queuedText
        ? "Replace queue"
        : queueOnSubmit
          ? "Queue"
          : "Send";
  const mateUnavailable =
    target === "mate" && (taskStatus === null || !ACTIVE_TASK_STATUS_TAGS.has(taskStatus.tag));
  const disableSubmit = agentCantSend || (!startupReady && target === "mate") || mateUnavailable;

  const filteredFiles =
    mentionQuery !== null
      ? worktreeFiles
          .filter((f) => f.toLowerCase().includes(mentionQuery.toLowerCase()))
          .slice(0, 10)
      : [];

  // r[ui.composer.file-mention]
  function getAtMentionQuery(value: string, cursorPos: number): string | null {
    const textBefore = value.slice(0, cursorPos);
    const match = textBefore.match(/@([a-zA-Z0-9/._-]*)$/);
    if (match) return match[1];
    return null;
  }

  function buildParts(value: string): PromptContentPart[] {
    const parts: PromptContentPart[] = [];
    if (value) parts.push({ tag: "Text", text: value });
    for (const img of attachedImages) {
      parts.push({ tag: "Image", mime_type: img.mimeType, data: img.data });
    }
    return parts;
  }

  async function sendNow(value: string, to: "captain" | "mate") {
    setLoading(true);
    setError(null);
    try {
      const client = await getShipClient();
      const parts = buildParts(value);
      if (to === "captain") {
        await client.promptCaptain(sessionId, parts);
      } else {
        await client.steer(sessionId, parts);
      }
      setAttachedImages((prev) => {
        for (const img of prev) URL.revokeObjectURL(img.objectUrl);
        return [];
      });
      setQueuedText(null);
      return true;
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      return false;
    } finally {
      setLoading(false);
    }
  }

  // Auto-flush queued captain messages when captain becomes idle
  useEffect(() => {
    if (!queuedText || loading || captainStateTag === "Working" || startupFailed) return;
    void (async () => {
      await sendNow(queuedText, "captain");
    })();
  }, [captainStateTag, loading, queuedText, startupFailed]);

  async function handleSubmit() {
    const raw = text.trim();
    if ((!raw && attachedImages.length === 0) || loading || disableSubmit) return;
    const { target: to, content } = parseTarget(raw);
    if (queueOnSubmit && to === "captain") {
      setQueuedText(content);
      setText("");
      setError(null);
      return;
    }
    if (await sendNow(content, to)) setText("");
  }

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    const style = getComputedStyle(el);
    const lineHeight = parseFloat(style.lineHeight) || 24;
    const paddingTop = parseFloat(style.paddingTop) || 0;
    const paddingBottom = parseFloat(style.paddingBottom) || 0;
    const maxHeight = lineHeight * 6 + paddingTop + paddingBottom;
    const newHeight = Math.min(el.scrollHeight, maxHeight);
    el.style.height = newHeight + "px";
    el.style.overflowY = el.scrollHeight > maxHeight ? "auto" : "hidden";
  }, [text]);

  function handleTextChange(e: React.ChangeEvent<HTMLTextAreaElement>) {
    const newText = e.target.value;
    setText(newText);
    const cursorPos = e.target.selectionStart ?? newText.length;
    setMentionQuery(getAtMentionQuery(newText, cursorPos));
    setSelectedIndex(0);
  }

  function insertMention(file: string) {
    const textarea = textareaRef.current;
    if (!textarea) return;
    const cursorPos = textarea.selectionStart ?? text.length;
    const textBefore = text.slice(0, cursorPos);
    const atIndex = textBefore.lastIndexOf("@");
    if (atIndex === -1) return;
    const newText = text.slice(0, atIndex) + "@" + file + " " + text.slice(cursorPos);
    setText(newText);
    setMentionQuery(null);
    const newCursor = atIndex + 1 + file.length + 1;
    requestAnimationFrame(() => {
      textarea.setSelectionRange(newCursor, newCursor);
      textarea.focus();
    });
  }

  function addImageFiles(files: FileList | File[]) {
    for (const file of Array.from(files)) {
      if (!file.type.startsWith("image/")) continue;
      const reader = new FileReader();
      reader.onload = () => {
        const data = new Uint8Array(reader.result as ArrayBuffer);
        const objectUrl = URL.createObjectURL(file);
        setAttachedImages((prev) => [
          ...prev,
          {
            id: `${Date.now()}-${Math.random()}`,
            mimeType: file.type,
            data,
            objectUrl,
            name: file.name || "pasted image",
          },
        ]);
      };
      reader.readAsArrayBuffer(file);
    }
  }

  function handlePaste(e: React.ClipboardEvent<HTMLTextAreaElement>) {
    const imageFiles: File[] = [];
    for (const item of Array.from(e.clipboardData.items)) {
      if (item.type.startsWith("image/")) {
        const file = item.getAsFile();
        if (file) imageFiles.push(file);
      }
    }
    if (imageFiles.length > 0) {
      e.preventDefault();
      addImageFiles(imageFiles);
    }
  }

  const showMateEntry = mentionQuery !== null && "mate".includes(mentionQuery.toLowerCase());
  const totalMentionItems = (showMateEntry ? 1 : 0) + filteredFiles.length;

  function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (mentionQuery !== null && totalMentionItems > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, totalMentionItems - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
        return;
      }
      if (e.key === "Enter" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        if (showMateEntry && selectedIndex === 0) {
          insertMention("mate");
        } else {
          insertMention(filteredFiles[selectedIndex - (showMateEntry ? 1 : 0)]);
        }
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setMentionQuery(null);
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      void handleSubmit();
      return;
    }
    if (e.key === "Escape" && activeStateTag === "Working") {
      e.preventDefault();
      void (async () => {
        const client = await getShipClient();
        await client.cancel(sessionId);
      })();
    }
  }

  return (
    <Flex className={composerRoot} direction="column" gap="2">
      {isDragOver && <div className={pageDropOverlay}>Drop image to attach</div>}
      {attachedImages.length > 0 && (
        <div className={attachedImageThumbList}>
          {attachedImages.map((img) => (
            <div key={img.id} className={attachedImageThumbWrapper}>
              <img src={img.objectUrl} alt={img.name} className={attachedImageThumb} />
              <button
                className={attachedImageRemove}
                onClick={() => {
                  setAttachedImages((prev) => {
                    const found = prev.find((i) => i.id === img.id);
                    if (found) URL.revokeObjectURL(found.objectUrl);
                    return prev.filter((i) => i.id !== img.id);
                  });
                }}
                aria-label={`Remove ${img.name}`}
              >
                ×
              </button>
            </div>
          ))}
        </div>
      )}

      <div className={composerInputWrapper} data-target={target === "mate" ? "mate" : undefined}>
        {mentionQuery !== null && totalMentionItems > 0 && (
          <div className={fileMentionPopup}>
            {showMateEntry && (
              <div
                className={fileMentionItem}
                data-special="mate"
                data-selected={selectedIndex === 0}
                onMouseDown={(e) => {
                  e.preventDefault();
                  insertMention("mate");
                }}
              >
                <Robot size={14} weight="regular" />
                mate
              </div>
            )}
            {filteredFiles.map((file, index) => (
              <div
                key={file}
                className={fileMentionItem}
                data-selected={(showMateEntry ? index + 1 : index) === selectedIndex}
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
          size="3"
          rows={1}
          placeholder="Steer the captain…"
          value={text}
          onChange={handleTextChange}
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          aria-label="Steer input"
        />
      </div>

      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        multiple
        style={{ display: "none" }}
        onChange={(e) => {
          if (e.target.files) addImageFiles(e.target.files);
          e.target.value = "";
        }}
      />

      <Flex className={composerActions} align="center" gap="2">
        <AgentStateChips captain={captain} mate={mate} />

        {(captainStateTag === "Working" || mateStateTag === "Working") && (
          <Flex align="center" gap="1" style={{ marginRight: "auto" }}>
            <div className={composerActivityDot} />
            <Text size="2" color="gray">
              {captainStateTag === "Working" && mateStateTag === "Working"
                ? "Both working"
                : captainStateTag === "Working"
                  ? "Captain working"
                  : "Mate working"}
            </Text>
            {activeAgent?.state.tag === "Working" && (
              <Box asChild style={{ opacity: 0.5, fontSize: "10px", fontFamily: "monospace" }}>
                <kbd>esc</kbd>
              </Box>
            )}
          </Flex>
        )}

        <Button
          size="3"
          variant="ghost"
          onClick={() => fileInputRef.current?.click()}
          disabled={loading}
          title="Attach image"
        >
          <PaperclipIcon />
        </Button>
        {mateUnavailable && (
          <Text size="1" color="gray">
            No active task — mate unavailable
          </Text>
        )}
        <Button
          size="2"
          onClick={() => void handleSubmit()}
          disabled={(!text.trim() && attachedImages.length === 0) || disableSubmit}
          loading={loading}
        >
          {submitLabel}{" "}
          <Box asChild style={{ opacity: 0.65, fontSize: "11px", fontFamily: "monospace" }}>
            <kbd>↵</kbd>
          </Box>
        </Button>
      </Flex>

      {error && (
        <Text size="2" color="red">
          {error}
        </Text>
      )}
    </Flex>
  );
}
