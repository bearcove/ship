import { useEffect } from "react";
import { useParams, useNavigate, Link } from "react-router-dom";
import { Box, Callout, Flex, IconButton, Spinner, Text } from "@radix-ui/themes";
import { ArrowLeft, List, Warning } from "@phosphor-icons/react";
import { useSession } from "../hooks/useSession";
import { useSessionState } from "../hooks/useSessionState";
import { UnifiedFeed } from "../components/UnifiedFeed";
import { UnifiedComposer } from "../components/UnifiedComposer";
import { SteerReview } from "../components/SteerReview";
import {
  agentRail,
  mobileNavBar,
  sessionViewRoot,
  unifiedFeedRoot,
} from "../styles/session-view.css";
import { AgentHeader } from "../components/AgentHeader";
import captainAvatar from "../assets/avatars/captain.png";
import mateAvatar from "../assets/avatars/mate.png";
import type { TaskRecord } from "../generated/ship";

// r[view.session]
// r[ui.layout.session-view]
// r[proto.hydration-flow]
export function SessionViewPage({
  debugMode,
  onOpenSidebar,
}: {
  debugMode: boolean;
  onOpenSidebar: () => void;
}) {
  const { sessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  // r[event.client.hydration-sequence]: Step 1 — structural state
  const { session, error } = useSession(sessionId ?? "");
  // r[event.client.hydration-sequence]: Step 2/3 — event subscription + replay
  const eventState = useSessionState(sessionId ?? "", session);

  // r[ui.keys.nav]
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key === "Escape") onOpenSidebar();
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onOpenSidebar]);

  if (error) {
    return (
      <Flex className={sessionViewRoot} align="center" justify="center" p="4">
        <Flex direction="column" gap="3" align="center" style={{ maxWidth: 400 }}>
          <Callout.Root color="red" size="2">
            <Callout.Icon>
              <Warning size={16} />
            </Callout.Icon>
            <Callout.Text>
              Session not found — it may have been created with an older format.
            </Callout.Text>
          </Callout.Root>
          <Text
            size="2"
            color="blue"
            style={{ cursor: "pointer", textDecoration: "underline" }}
            onClick={() => navigate("/")}
          >
            Back to sessions
          </Text>
        </Flex>
      </Flex>
    );
  }

  if (!session) {
    return (
      <Flex className={sessionViewRoot} align="center" justify="center">
        <Spinner size="3" />
      </Flex>
    );
  }

  const captain = eventState.captain ?? session.captain;
  const mate = eventState.mate ?? session.mate;
  const startupState = eventState.startupState ?? session.startup_state;
  const isReplaying = eventState.phase !== "live";
  const replayLabel = eventState.connected
    ? eventState.replayEventCount > 0
      ? `Replaying ${eventState.replayEventCount} event${eventState.replayEventCount === 1 ? "" : "s"}…`
      : "Connected — waiting for replay…"
    : "Waiting for reconnect…";
  const liveTask: TaskRecord | null =
    eventState.currentTaskId &&
    eventState.currentTaskTitle &&
    eventState.currentTaskDescription &&
    eventState.currentTaskStatus
      ? {
          id: eventState.currentTaskId,
          title: eventState.currentTaskTitle,
          description: eventState.currentTaskDescription,
          status: eventState.currentTaskStatus,
        }
      : session.current_task;

  return (
    <Flex className={sessionViewRoot}>
      <Box className={mobileNavBar}>
        <Flex align="center" gap="2" px="2" py="2">
          <IconButton
            variant="ghost"
            color="gray"
            size="2"
            onClick={onOpenSidebar}
            aria-label="Open sidebar"
          >
            <List size={18} />
          </IconButton>
          <Link to="/" style={{ color: "var(--gray-11)", display: "flex", alignItems: "center" }}>
            <ArrowLeft size={18} />
          </Link>
        </Flex>
      </Box>

      <Flex style={{ flex: 1, overflow: "hidden", minHeight: 0 }}>
        <Box className={unifiedFeedRoot} style={{ flex: 1 }}>
          <UnifiedFeed
            sessionId={session.id}
            captain={captain}
            mate={mate}
            blocks={eventState.unifiedBlocks.blocks}
            startupState={startupState}
            taskStatus={liveTask?.status ?? null}
            userAvatarUrl={session.user_avatar_url}
            loading={isReplaying}
            loadingLabel={replayLabel}
            debugMode={debugMode}
          />
          <UnifiedComposer
            sessionId={session.id}
            captain={captain}
            mate={mate}
            startupState={startupState}
            taskStatus={liveTask?.status ?? null}
          />
        </Box>
        {(captain ?? mate) && (
          <Box className={agentRail}>
            {captain && (
              <AgentHeader sessionId={session.id} agent={captain} avatarSrc={captainAvatar} />
            )}
            {mate && <AgentHeader sessionId={session.id} agent={mate} avatarSrc={mateAvatar} />}
          </Box>
        )}
      </Flex>

      {session.pending_steer && (
        <SteerReview
          sessionId={session.id}
          steerText={session.pending_steer}
          onDismiss={() => {}}
        />
      )}
    </Flex>
  );
}
