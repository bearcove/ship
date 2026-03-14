import { Badge, Flex } from "@radix-ui/themes";
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

  const isRecording = transcription.state.tag === "recording";
  const label = isRecording ? "Recording" : "Transcribing";

  return (
    <Badge
      size="1"
      color={isRecording ? "red" : "amber"}
      variant={isRecording ? "solid" : "soft"}
      aria-label={`${label} for this session`}
      data-testid={`session-recording-badge-${sessionId}`}
      style={{ flexShrink: 0, whiteSpace: "nowrap" }}
    >
      <Flex align="center" gap="1">
        <span
          aria-hidden
          style={{
            width: 6,
            height: 6,
            borderRadius: "50%",
            background: isRecording ? "currentColor" : "var(--amber-9)",
            opacity: isRecording ? 0.9 : 0.6,
          }}
        />
        <Microphone size={10} weight={isRecording ? "fill" : "regular"} />
        {!compact ? label : null}
      </Flex>
    </Badge>
  );
}
