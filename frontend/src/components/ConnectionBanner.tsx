import { Callout } from "@radix-ui/themes";
import { WifiSlash } from "@phosphor-icons/react";

interface Props {
  connected: boolean;
  phase: "loading" | "replaying" | "live";
  disconnectReason: string | null;
  replayEventCount: number;
  connectionAttempt: number;
  lastSeq: number | null;
  lastEventKind: string | null;
}

// r[ui.error.connection]
export function ConnectionBanner({
  connected,
  phase,
  disconnectReason,
  replayEventCount,
  connectionAttempt,
  lastSeq,
  lastEventKind,
}: Props) {
  if (connected && phase === "live") return null;

  const commonStyle = {
    borderRadius: 0,
    borderLeft: "none",
    borderRight: "none",
    borderTop: "none",
  } as const;

  if (!connected) {
    return (
      <Callout.Root color="red" size="1" style={commonStyle}>
        <Callout.Icon>
          <WifiSlash size={16} />
        </Callout.Icon>
        <Callout.Text>
          {disconnectReason
            ? `Connection lost — attempting to reconnect. ${disconnectReason}`
            : "Connection lost — attempting to reconnect…"}
          {lastSeq !== null ? ` Last event: ${lastEventKind ?? "unknown"} at seq ${lastSeq}.` : ""}
        </Callout.Text>
      </Callout.Root>
    );
  }

  return (
    <Callout.Root color="blue" size="1" style={commonStyle}>
      <Callout.Text>
        {replayEventCount > 0
          ? `Connected — replaying events (${replayEventCount} received on attempt ${connectionAttempt}).`
          : `Connected — waiting for replay on attempt ${connectionAttempt}.`}
      </Callout.Text>
    </Callout.Root>
  );
}
