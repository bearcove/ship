import { useEffect, useState } from "react";
import { useParams, useNavigate, Link } from "react-router-dom";
import { Box, Callout, Flex, SegmentedControl, Spinner, Text } from "@radix-ui/themes";
import { ArrowLeft, Warning } from "@phosphor-icons/react";
import { useSession } from "../hooks/useSession";
import { useSessionState } from "../hooks/useSessionState";
import { AgentHeader } from "../components/AgentHeader";
import { AgentPanel } from "../components/AgentPanel";
import { SteerReview } from "../components/SteerReview";
import {
  sessionViewRoot,
  desktopGrid,
  panelColumn,
  mobileNavBar,
  mobilePanel,
} from "../styles/session-view.css";
import type { TaskRecord } from "../generated/ship";

// r[view.session]
// r[ui.layout.session-view]
// r[proto.hydration-flow]
export function SessionViewPage({ debugMode }: { debugMode: boolean }) {
  const { sessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  // r[event.client.hydration-sequence]: Step 1 — structural state
  const { session, error } = useSession(sessionId ?? "");
  // r[event.client.hydration-sequence]: Step 2/3 — event subscription + replay
  const eventState = useSessionState(sessionId ?? "", session);
  const [mobileTab, setMobileTab] = useState<"captain" | "mate">("captain");

  // r[ui.keys.nav]
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key === "1") setMobileTab("captain");
      if (e.key === "2") setMobileTab("mate");
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

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
        <Flex align="center" gap="3" px="3" py="2">
          <Link to="/" style={{ color: "var(--gray-11)", display: "flex", alignItems: "center" }}>
            <ArrowLeft size={18} />
          </Link>
          <SegmentedControl.Root
            size="1"
            value={mobileTab}
            onValueChange={(v) => setMobileTab(v as "captain" | "mate")}
          >
            <SegmentedControl.Item value="captain">Captain</SegmentedControl.Item>
            <SegmentedControl.Item value="mate">Mate</SegmentedControl.Item>
          </SegmentedControl.Root>
        </Flex>
      </Box>

      <Box style={{ flex: 1, overflow: "hidden", display: "flex", flexDirection: "column" }}>
        <Box className={desktopGrid} style={{ flex: 1 }}>
          <Box className={panelColumn}>
            <AgentHeader sessionId={session.id} agent={captain} />
            <AgentPanel
              sessionId={session.id}
              agent={captain}
              blocks={eventState.captainBlocks.blocks}
              debugMode={debugMode}
              loading={isReplaying}
              loadingLabel={replayLabel}
              startupState={startupState}
              taskStatus={liveTask?.status ?? null}
            />
          </Box>
          <Box className={panelColumn}>
            <AgentHeader sessionId={session.id} agent={mate} />
            <AgentPanel
              sessionId={session.id}
              agent={mate}
              blocks={eventState.mateBlocks.blocks}
              debugMode={debugMode}
              loading={isReplaying}
              loadingLabel={replayLabel}
              startupState={startupState}
              taskStatus={liveTask?.status ?? null}
            />
          </Box>
        </Box>

        <Box className={mobilePanel}>
          {mobileTab === "captain" ? (
            <>
              <AgentHeader sessionId={session.id} agent={captain} />
              <AgentPanel
                sessionId={session.id}
                agent={captain}
                blocks={eventState.captainBlocks.blocks}
                debugMode={debugMode}
                loading={isReplaying}
                loadingLabel={replayLabel}
                startupState={startupState}
                taskStatus={liveTask?.status ?? null}
              />
            </>
          ) : (
            <>
              <AgentHeader sessionId={session.id} agent={mate} />
              <AgentPanel
                sessionId={session.id}
                agent={mate}
                blocks={eventState.mateBlocks.blocks}
                debugMode={debugMode}
                loading={isReplaying}
                loadingLabel={replayLabel}
                startupState={startupState}
                taskStatus={liveTask?.status ?? null}
              />
            </>
          )}
        </Box>
      </Box>

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
