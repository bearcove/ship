import { useEffect, useRef, useState } from "react";
import { Box, Flex, Text, TextArea } from "@radix-ui/themes";
import {
  ArrowUp,
  Microphone,
  PaperclipIcon,
  Robot,
  Spinner,
  Stop,
  Warning,
  X,
} from "@phosphor-icons/react";
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
  composerActivityDot,
  composerEscHint,
  composerInlineBtn,
  composerInput,
  composerInputWideRight,
  composerInputWrapper,
  composerOverlay,
  composerRoot,
  composerStatusRow,
  fileMentionItem,
  fileMentionPopup,
  pageDropOverlay,
  transcriptPreview,
} from "../styles/session-view.css";
import { Waveform } from "./Waveform";
import { useWorktreeDiffStats } from "../hooks/useWorktreeDiffStats";
import { useDocumentDrop } from "../hooks/useDocumentDrop";
import { useTranscription } from "../hooks/useTranscription";

interface AttachedImage {
  id: string;
  mimeType: string;
  data: Uint8Array;
  objectUrl: string;
  name: string;
}

const SUPPORTED_IMAGE_TYPES = new Set(["image/png", "image/jpeg", "image/gif", "image/webp"]);

async function convertToSupportedFormat(
  file: File,
): Promise<{ data: Uint8Array; mimeType: string; objectUrl: string }> {
  if (SUPPORTED_IMAGE_TYPES.has(file.type)) {
    const buffer = await file.arrayBuffer();
    return {
      data: new Uint8Array(buffer),
      mimeType: file.type,
      objectUrl: URL.createObjectURL(file),
    };
  }

  // Unsupported format (e.g. HEIC) — decode via Image element and re-encode as PNG
  const srcUrl = URL.createObjectURL(file);
  try {
    const img = await new Promise<HTMLImageElement>((resolve, reject) => {
      const el = new Image();
      el.onload = () => resolve(el);
      el.onerror = () => reject(new Error(`Failed to decode image: ${file.name}`));
      el.src = srcUrl;
    });

    const canvas = document.createElement("canvas");
    canvas.width = img.naturalWidth;
    canvas.height = img.naturalHeight;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("Canvas 2D context unavailable");
    ctx.drawImage(img, 0, 0);

    const blob = await new Promise<Blob>((resolve, reject) => {
      canvas.toBlob((b) => {
        if (b) resolve(b);
        else reject(new Error("Canvas toBlob failed"));
      }, "image/png");
    });

    const buffer = await blob.arrayBuffer();
    const objectUrl = URL.createObjectURL(blob);
    return { data: new Uint8Array(buffer), mimeType: "image/png", objectUrl };
  } finally {
    URL.revokeObjectURL(srcUrl);
  }
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

function formatElapsed(ms: number): string {
  const totalSec = Math.floor(ms / 1000);
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${min}:${sec.toString().padStart(2, "0")}`;
}

// r[ui.keys.steer-send]
// r[ui.composer.image-attach]
// r[view.agent-panel.activity]
export function UnifiedComposer({ sessionId, captain, mate, startupState, taskStatus }: Props) {
  const diffStats = useWorktreeDiffStats(sessionId);
  const [text, setText] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mentionQuery, setMentionQuery] = useState<string | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [attachedImages, setAttachedImages] = useState<AttachedImage[]>([]);
  const [sendAfterTranscription, setSendAfterTranscription] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const transcriptPreviewRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [fileMatches, setFileMatches] = useState<string[]>([]);
  const isDragOver = useDocumentDrop(addImageFiles);
  const transcription = useTranscription();

  // Track the text that existed before transcription started, so we can
  // prepend it to the final transcription result.
  const preTranscriptionTextRef = useRef<string | null>(null);

  // Capture pre-transcription text when recording starts
  if (transcription.state.tag === "recording" && preTranscriptionTextRef.current === null) {
    preTranscriptionTextRef.current = text;
  }

  // When transcription returns to idle, commit the final text and optionally auto-submit
  const prevTranscriptionTag = useRef(transcription.state.tag);
  useEffect(() => {
    const wasProcessing = prevTranscriptionTag.current !== "idle";
    prevTranscriptionTag.current = transcription.state.tag;
    if (!wasProcessing || transcription.state.tag !== "idle") return;

    const prefix = preTranscriptionTextRef.current ?? "";
    preTranscriptionTextRef.current = null;

    const finalText = transcription.result?.text
      ? prefix
        ? prefix + " " + transcription.result.text
        : transcription.result.text
      : prefix;

    if (sendAfterTranscription) {
      setSendAfterTranscription(false);
      const trimmed = finalText.trim();
      if (trimmed || attachedImages.length > 0) {
        const { target: to, content } = parseTarget(trimmed);
        void sendNow(content, to).then((success) => {
          if (success) setText("");
        });
      }
    } else {
      setText(finalText);
    }
  }, [transcription.state.tag]); // eslint-disable-line react-hooks/exhaustive-deps

  const { target } = parseTarget(text);
  const activeAgent = target === "captain" ? captain : mate;
  const captainStateTag = captain?.state.tag ?? "Idle";
  const mateStateTag = mate?.state.tag ?? "Idle";
  const activeStateTag = target === "captain" ? captainStateTag : mateStateTag;

  const startupReady = startupState === null || startupState.tag === "Ready";
  const startupFailed = startupState?.tag === "Failed";
  const agentCantSend =
    activeStateTag === "ContextExhausted" || activeStateTag === "Error" || startupFailed;

  const submitLabel = target === "mate" ? "Steer mate" : "Send";
  const mateUnavailable =
    target === "mate" && (taskStatus === null || !ACTIVE_TASK_STATUS_TAGS.has(taskStatus.tag));
  const disableSubmit = agentCantSend || (!startupReady && target === "mate") || mateUnavailable;

  useEffect(() => {
    if (mentionQuery === null) {
      setFileMatches([]);
      return;
    }
    let active = true;
    async function fetchFiles() {
      const client = await getShipClient();
      const list = await client.listWorktreeFiles(sessionId, mentionQuery ?? "");
      if (active) setFileMatches(list);
    }
    void fetchFiles();
    return () => {
      active = false;
    };
  }, [sessionId, mentionQuery]);

  const filteredFiles = fileMatches;

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
      return true;
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      return false;
    } finally {
      setLoading(false);
    }
  }

  async function handleSubmit() {
    const raw = text.trim();
    if ((!raw && attachedImages.length === 0) || loading || disableSubmit) return;
    const { target: to, content } = parseTarget(raw);
    if (await sendNow(content, to)) setText("");
  }

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    const style = getComputedStyle(el);
    const lineHeight = parseFloat(style.lineHeight) || 24;
    const paddingTop = parseFloat(style.paddingTop) || 0;
    const paddingBottom = parseFloat(style.paddingBottom) || 0;
    const minHeight = lineHeight + paddingTop + paddingBottom;
    const maxHeight = lineHeight * 6 + paddingTop + paddingBottom;
    // Reset to 0 so scrollHeight reflects actual content, not previous height
    el.style.height = "0px";
    const contentHeight = Math.max(el.scrollHeight, minHeight);
    const newHeight = Math.min(contentHeight, maxHeight);
    el.style.height = newHeight + "px";
    el.style.overflowY = contentHeight > maxHeight ? "auto" : "hidden";
  }, [text]);

  useEffect(() => {
    const el = transcriptPreviewRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [transcription.result]);

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
      void convertToSupportedFormat(file)
        .then(({ data, mimeType, objectUrl }) => {
          setAttachedImages((prev) => [
            ...prev,
            {
              id: `${Date.now()}-${Math.random()}`,
              mimeType,
              data,
              objectUrl,
              name: file.name || "pasted image",
            },
          ]);
        })
        .catch((err) => {
          console.warn(`Skipping image "${file.name}": could not decode`, err);
        });
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
      const isTouchDevice = window.matchMedia("(pointer: coarse)").matches;
      if (!isTouchDevice) {
        e.preventDefault();
        void handleSubmit();
      }
      return;
    }
    if (e.key === "Escape" && (captainStateTag === "Working" || mateStateTag === "Working")) {
      e.preventDefault();
      void (async () => {
        const client = await getShipClient();
        await client.stopAgents(sessionId);
      })();
    }
  }

  const hasContent = text.trim().length > 0 || attachedImages.length > 0;
  const isRecording = transcription.state.tag === "recording";
  const isProcessing = transcription.state.tag === "processing";
  const isWorking = captainStateTag === "Working" || mateStateTag === "Working";

  return (
    <Flex className={composerRoot} direction="column" gap="2">
      {isDragOver && <div className={pageDropOverlay}>Drop image to attach</div>}

      {(isWorking || mateUnavailable || diffStats) && (
        <Flex
          className={composerStatusRow}
          align="center"
          gap="2"
          data-testid="composer-status-row"
          data-working-anchor={isWorking ? "left" : undefined}
        >
          <AgentStateChips captain={captain} mate={mate} />
          {mateUnavailable && (
            <Text size="1" color="gray">
              No active task
            </Text>
          )}
          {diffStats && (
            <Flex
              align="center"
              gap="2"
              data-testid="composer-diff-stats"
              style={{ marginLeft: "auto", fontFamily: "var(--code-font-family)" }}
            >
              <Text
                size="1"
                color="gray"
                style={{
                  fontFamily: "var(--code-font-family)",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                  maxWidth: 140,
                }}
              >
                {diffStats.branch_name}
              </Text>
              {diffStats.files_changed > 0n && (
                <>
                  <Text size="1" style={{ color: "var(--green-10)" }}>
                    +{String(diffStats.lines_added)}
                  </Text>
                  <Text size="1" style={{ color: "var(--red-10)" }}>
                    &minus;{String(diffStats.lines_removed)}
                  </Text>
                </>
              )}
            </Flex>
          )}
        </Flex>
      )}

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

      {isRecording && transcription.result && (
        <div ref={transcriptPreviewRef} className={transcriptPreview}>
          {transcription.result.text}
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

        {/* Left slot */}
        <button
          type="button"
          className={composerInlineBtn}
          data-pos="left"
          onClick={() => fileInputRef.current?.click()}
          disabled={loading || isRecording}
          title="Attach image"
        >
          <PaperclipIcon size={18} />
        </button>

        {/* Textarea — always present, hidden behind overlay when recording/processing */}
        <TextArea
          ref={textareaRef}
          className={`${composerInput}${isRecording ? ` ${composerInputWideRight}` : ""}`}
          size="3"
          rows={1}
          placeholder="Steer the captain…"
          value={text}
          onChange={handleTextChange}
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          aria-label="Steer input"
          style={{ visibility: isRecording || isProcessing ? "hidden" : undefined }}
        />

        {/* Overlay: waveform during recording, spinner during processing */}
        {isRecording && transcription.analyser && (
          <div className={composerOverlay}>
            <Text
              size="2"
              color="gray"
              style={{
                fontVariantNumeric: "tabular-nums",
                flexShrink: 0,
                width: "4ch",
                textAlign: "left",
              }}
            >
              {formatElapsed(
                transcription.state.tag === "recording" ? transcription.state.elapsed : 0,
              )}
            </Text>
            <Waveform analyser={transcription.analyser} />
          </div>
        )}
        {isProcessing && (
          <div className={composerOverlay}>
            <Spinner size={16} />
            <Text size="2" color="gray">
              {sendAfterTranscription ? "Sending…" : "Transcribing…"}
            </Text>
          </div>
        )}

        {/* Right slot */}
        {isRecording ? (
          <>
            <button
              type="button"
              className={composerInlineBtn}
              data-pos="right-2"
              onClick={() => void transcription.stopRecording()}
              title="Stop recording"
            >
              <Stop size={18} weight="fill" />
            </button>
            <button
              type="button"
              className={composerInlineBtn}
              data-pos="right"
              data-variant="solid"
              onClick={() => {
                setSendAfterTranscription(true);
                void transcription.stopRecording();
              }}
              disabled={disableSubmit}
              title="Stop and send"
            >
              <ArrowUp size={20} weight="bold" />
            </button>
          </>
        ) : isProcessing ? (
          <button
            type="button"
            className={composerInlineBtn}
            data-pos="right"
            onClick={() => void transcription.cancelRecording()}
            title="Cancel"
          >
            <X size={18} />
          </button>
        ) : text.trim().length > 0 ? (
          <button
            type="button"
            className={composerInlineBtn}
            data-pos="right"
            data-variant="solid"
            onClick={() => void handleSubmit()}
            disabled={disableSubmit || loading}
            title={submitLabel}
          >
            <ArrowUp size={20} weight="bold" />
          </button>
        ) : (
          <button
            type="button"
            className={composerInlineBtn}
            data-pos="right"
            onClick={() => void transcription.startRecording()}
            disabled={loading}
            title="Voice input"
          >
            <Microphone size={20} />
          </button>
        )}
      </div>

      {error && (
        <Text size="2" color="red">
          {error}
        </Text>
      )}
    </Flex>
  );
}
