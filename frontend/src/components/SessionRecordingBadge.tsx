import { Microphone } from "@phosphor-icons/react";
import { useTranscription } from "../context/TranscriptionContext";

interface Props {
  sessionId: string;
  compact?: boolean;
}

export function SessionRecordingBadge({ sessionId, compact = false }: Props) {
  const transcription = useTranscription();
  if (transcription.targetSessionId !== sessionId) return null;
  if (transcription.state.tag !== "recording" && transcription.state.tag !== "processing") return null;

  const icon = (
    <Microphone
      size={14}
      weight="fill"
      style={{ color: "var(--red-9)" }}
      aria-label="Recording"
      data-testid={`session-recording-badge-${sessionId}`}
    />
  );

  if (compact) {
    return (
      <span
        style={{
          position: "absolute",
          right: "var(--space-3)",
          top: "50%",
          transform: "translateY(-50%)",
          display: "flex",
          alignItems: "center",
          pointerEvents: "none",
        }}
      >
        {icon}
      </span>
    );
  }

  return icon;
}
