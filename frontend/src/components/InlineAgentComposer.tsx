import { useEffect, useRef, useState } from "react";
import { Box, Button, Flex, Text, TextArea } from "@radix-ui/themes";
import { getShipClient } from "../api/client";
import type { PromptContentPart, Role, SessionStartupState, TaskStatus } from "../generated/ship";
import {
  attachedImageRemove,
  attachedImageThumb,
  attachedImageThumbList,
  attachedImageThumbWrapper,
  composerActions,
  composerActivityDot,
  composerDropIndicator,
  composerInput,
  composerInputWrapper,
  composerRoot,
  fileMentionItem,
  fileMentionPopup,
} from "../styles/session-view.css";
import { useWorktreeFiles } from "../hooks/useWorktreeFiles";

interface AttachedImage {
  id: string;
  mimeType: string;
  data: Uint8Array;
  objectUrl: string;
  name: string;
}

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
      disableInput: false,
      disableSubmit: false,
      queueOnSubmit: true,
      submitLabel: "Queue",
    };
  }

  if (agentStateTag === "ContextExhausted") {
    return {
      disableInput: false,
      disableSubmit: true,
      queueOnSubmit: false,
      submitLabel: "Send",
    };
  }

  if (agentStateTag === "Error") {
    return {
      disableInput: false,
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
// r[ui.composer.image-attach]
// r[view.agent-panel.activity]
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
  const [attachedImages, setAttachedImages] = useState<AttachedImage[]>([]);
  const [isDragOver, setIsDragOver] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const status = getStatusCopy(role, agentStateTag, startupState, taskStatus, queuedText);
  const worktreeFiles = useWorktreeFiles(sessionId);

  const filteredFiles =
    mentionQuery !== null
      ? worktreeFiles
          .filter((f) => f.toLowerCase().includes(mentionQuery.toLowerCase()))
          .slice(0, 10)
      : [];

  function buildParts(value: string): PromptContentPart[] {
    const parts: PromptContentPart[] = [];
    if (value) {
      parts.push({ tag: "Text", text: value });
    }
    for (const img of attachedImages) {
      parts.push({ tag: "Image", mime_type: img.mimeType, data: img.data });
    }
    return parts;
  }

  async function sendNow(value: string) {
    setLoading(true);
    setError(null);
    try {
      const client = await getShipClient();
      const parts = buildParts(value);
      if (role.tag === "Captain") {
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

  function addImageFiles(files: FileList | File[]) {
    for (const file of Array.from(files)) {
      if (!file.type.startsWith("image/")) continue;
      const reader = new FileReader();
      reader.onload = () => {
        const buffer = reader.result as ArrayBuffer;
        const data = new Uint8Array(buffer);
        const objectUrl = URL.createObjectURL(file);
        const id = `${Date.now()}-${Math.random()}`;
        setAttachedImages((prev) => [
          ...prev,
          { id, mimeType: file.type, data, objectUrl, name: file.name || "pasted image" },
        ]);
      };
      reader.readAsArrayBuffer(file);
    }
  }

  function handleDragOver(event: React.DragEvent) {
    event.preventDefault();
    setIsDragOver(true);
  }

  function handleDragLeave() {
    setIsDragOver(false);
  }

  function handleDrop(event: React.DragEvent) {
    event.preventDefault();
    setIsDragOver(false);
    const files = event.dataTransfer.files;
    if (files.length > 0) addImageFiles(files);
  }

  function removeImage(id: string) {
    setAttachedImages((prev) => {
      const img = prev.find((i) => i.id === id);
      if (img) URL.revokeObjectURL(img.objectUrl);
      return prev.filter((i) => i.id !== id);
    });
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
    if ((!value && attachedImages.length === 0) || loading || status.disableSubmit) return;

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
    const newText = text.slice(0, atIndex) + "@" + file + " " + text.slice(cursorPos);
    setText(newText);
    setMentionQuery(null);
    const newCursorPos = atIndex + 1 + file.length + 1;
    requestAnimationFrame(() => {
      textarea.setSelectionRange(newCursorPos, newCursorPos);
      textarea.focus();
    });
  }

  function handlePaste(event: React.ClipboardEvent<HTMLTextAreaElement>) {
    const imageFiles: File[] = [];
    for (const item of Array.from(event.clipboardData.items)) {
      if (item.type.startsWith("image/")) {
        const file = item.getAsFile();
        if (file) imageFiles.push(file);
      }
    }
    if (imageFiles.length > 0) {
      event.preventDefault();
      addImageFiles(imageFiles);
    }
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

    if (event.key === "Enter" && !event.shiftKey && !event.metaKey && !event.ctrlKey) {
      event.preventDefault();
      void handleSubmit();
      return;
    }

    if (event.key === "Escape" && agentStateTag === "Working") {
      event.preventDefault();
      void handleCancel();
    }
  }

  async function handleCancel() {
    try {
      const client = await getShipClient();
      await client.cancel(sessionId);
    } catch (err) {
      console.error("[ship/session] failed to cancel", { sessionId, error: err });
    }
  }

  return (
    <Flex
      className={composerRoot}
      direction="column"
      gap="2"
      onDragOver={handleDragOver}
      onDragEnter={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      data-drag-over={isDragOver}
    >
      {attachedImages.length > 0 && (
        <div className={attachedImageThumbList}>
          {attachedImages.map((img) => (
            <div key={img.id} className={attachedImageThumbWrapper}>
              <img src={img.objectUrl} alt={img.name} className={attachedImageThumb} />
              <button
                className={attachedImageRemove}
                onClick={() => removeImage(img.id)}
                aria-label={`Remove ${img.name}`}
              >
                ×
              </button>
            </div>
          ))}
        </div>
      )}
      <div className={composerInputWrapper}>
        {isDragOver && <div className={composerDropIndicator}>Drop image here</div>}
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
          onPaste={handlePaste}
          aria-label={role.tag === "Captain" ? "Captain steer input" : "Mate steer input"}
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
      <Flex className={composerActions} align="center" justify="end" gap="2">
        {agentStateTag === "Working" && (
          <Flex align="center" gap="1">
            <div className={composerActivityDot} />
            <Text size="1" color="gray">
              Working
            </Text>
            <Box asChild style={{ opacity: 0.5, fontSize: "10px", fontFamily: "monospace" }}>
              <kbd>esc</kbd>
            </Box>
          </Flex>
        )}
        <Button
          size="1"
          variant="ghost"
          onClick={() => fileInputRef.current?.click()}
          disabled={status.disableInput || loading}
          title="Attach image"
        >
          ⌗
        </Button>
        <Button
          size="1"
          onClick={() => void handleSubmit()}
          disabled={(!text.trim() && attachedImages.length === 0) || status.disableSubmit}
          loading={loading}
        >
          {status.submitLabel}{" "}
          <Box asChild style={{ opacity: 0.65, fontSize: "11px", fontFamily: "monospace" }}>
            <kbd>↵</kbd>
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
