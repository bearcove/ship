import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { Box, Callout, Flex, IconButton, Spinner, Switch, Tabs, Text } from "@radix-ui/themes";
import { Bug, Clock, SpeakerHigh, SpeakerSlash } from "@phosphor-icons/react";
import { useSoundEnabled } from "../context/SoundContext";
import { useSession } from "../hooks/useSession";
import { useSessionState } from "../hooks/useSessionState";
import { AgentHeader } from "../components/AgentHeader";
import { AgentPanel } from "../components/AgentPanel";
import { TaskBar } from "../components/TaskBar";
import { SteerReview } from "../components/SteerReview";
import { ConnectionBanner } from "../components/ConnectionBanner";
import {
  autonomyBadge,
  autonomyControls,
  sessionBreadcrumbButton,
  sessionBreadcrumbs,
  sessionBreadcrumbSeparator,
  sessionViewRoot,
  sessionTopBar,
  sessionBranch,
  desktopGrid,
  panelColumn,
  mobileTabs,
  mobilePanel,
  idleBanner,
} from "../styles/session-view.css";
import type { TaskRecord, TaskStatus } from "../generated/ship";

function getIdleMessage(
  startupStateTag: string | null,
  taskStatus: TaskStatus | null,
  mateAwaitingPermission: boolean,
): string | null {
  if (startupStateTag === "Pending" || startupStateTag === "Running") {
    return "Session startup is in progress.";
  }
  if (startupStateTag === "Failed") {
    return "Session startup failed.";
  }
  if (taskStatus?.tag === "ReviewPending")
    return "Mate finished — review the work or send the next steer.";
  if (taskStatus?.tag === "SteerPending")
    return "Captain drafted the next steer — review it above or override it with your own.";
  if (mateAwaitingPermission) return "Mate needs permission approval before it can continue.";
  return null;
}

function readDebugPreference(): boolean {
  if (
    typeof window === "undefined" ||
    !("localStorage" in window) ||
    typeof window.localStorage?.getItem !== "function"
  ) {
    return false;
  }
  return window.localStorage.getItem("ship.debug") === "1";
}

function writeDebugPreference(enabled: boolean) {
  if (
    typeof window === "undefined" ||
    !("localStorage" in window) ||
    typeof window.localStorage?.setItem !== "function"
  ) {
    return;
  }
  window.localStorage.setItem("ship.debug", enabled ? "1" : "0");
}

// r[view.session]
// r[ui.layout.session-view]
// r[proto.hydration-flow]
export function SessionViewPage() {
  const { sessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  const { soundEnabled, setSoundEnabled } = useSoundEnabled();
  // r[event.client.hydration-sequence]: Step 1 — structural state
  const session = useSession(sessionId ?? "");
  // r[event.client.hydration-sequence]: Step 2/3 — event subscription + replay
  const eventState = useSessionState(sessionId ?? "", session);
  const [autonomous, setAutonomous] = useState(false);
  const [mobileTab, setMobileTab] = useState<"captain" | "mate">("captain");
  const [debugMode, setDebugMode] = useState(readDebugPreference);

  useEffect(() => {
    if (session) setAutonomous(session.autonomy_mode.tag === "Autonomous");
  }, [session]);

  useEffect(() => {
    writeDebugPreference(debugMode);
  }, [debugMode]);

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
  const mateAwaitingPermission = mate.state.tag === "AwaitingPermission";
  const idle = getIdleMessage(
    startupState?.tag ?? null,
    eventState.currentTaskStatus ?? session.current_task?.status ?? null,
    mateAwaitingPermission,
  );
  const isReplaying = eventState.phase !== "live";
  const replayLabel = eventState.connected
    ? eventState.replayEventCount > 0
      ? `Replaying ${eventState.replayEventCount} event${eventState.replayEventCount === 1 ? "" : "s"}…`
      : "Connected — waiting for replay…"
    : "Waiting for reconnect…";
  const liveTask: TaskRecord | null =
    eventState.currentTaskId && eventState.currentTaskDescription && eventState.currentTaskStatus
      ? {
          id: eventState.currentTaskId,
          description: eventState.currentTaskDescription,
          status: eventState.currentTaskStatus,
        }
      : session.current_task;

  return (
    <Flex className={sessionViewRoot}>
      <Flex className={sessionTopBar}>
        <Flex className={sessionBreadcrumbs}>
          <button type="button" className={sessionBreadcrumbButton} onClick={() => navigate("/")}>
            ship
          </button>
          <Text className={sessionBreadcrumbSeparator}>::</Text>
          <button
            type="button"
            className={sessionBreadcrumbButton}
            onClick={() => navigate(`/?project=${encodeURIComponent(session.project)}`)}
          >
            {session.project}
          </button>
          <Text className={sessionBreadcrumbSeparator}>::</Text>
          <Text className={sessionBranch}>{session.branch_name}</Text>
        </Flex>
        {/* r[ui.autonomy.toggle] */}
        <Flex className={autonomyControls}>
          <Switch
            size="2"
            checked={autonomous}
            onCheckedChange={setAutonomous}
            aria-label={autonomous ? "Autonomous mode enabled" : "Human-in-the-loop mode enabled"}
          />
          <Box
            className={autonomyBadge}
            aria-hidden="true"
            data-mode={autonomous ? "auto" : "hitl"}
          />
        </Flex>
        <IconButton
          variant={debugMode ? "solid" : "ghost"}
          color={debugMode ? "amber" : "gray"}
          size="2"
          onClick={() => setDebugMode((enabled) => !enabled)}
          aria-label={debugMode ? "Disable debug mode" : "Enable debug mode"}
        >
          <Bug size={18} />
        </IconButton>
        <IconButton
          variant="ghost"
          size="2"
          onClick={() => setSoundEnabled(!soundEnabled)}
          aria-label={soundEnabled ? "Mute sounds" : "Unmute sounds"}
        >
          {soundEnabled ? <SpeakerHigh size={18} /> : <SpeakerSlash size={18} />}
        </IconButton>
      </Flex>

      <ConnectionBanner
        connected={eventState.connected}
        phase={eventState.phase}
        disconnectReason={eventState.disconnectReason}
        replayEventCount={eventState.replayEventCount}
        connectionAttempt={eventState.connectionAttempt}
        lastSeq={eventState.lastSeq}
        lastEventKind={eventState.lastEventKind}
      />

      <Box className={mobileTabs} px="3" pt="2">
        <Tabs.Root value={mobileTab} onValueChange={(v) => setMobileTab(v as "captain" | "mate")}>
          <Tabs.List>
            <Tabs.Trigger value="captain">Captain</Tabs.Trigger>
            <Tabs.Trigger value="mate">Mate</Tabs.Trigger>
          </Tabs.List>
        </Tabs.Root>
      </Box>

      {/* r[ui.idle.banner] */}
      {idle && (
        <Callout.Root color="amber" size="1" className={idleBanner}>
          <Callout.Icon>
            <Clock size={14} />
          </Callout.Icon>
          <Callout.Text>{idle}</Callout.Text>
        </Callout.Root>
      )}
      {startupState && startupState.tag === "Failed" && (
        <Callout.Root color="red" size="1" className={idleBanner}>
          <Callout.Text>{startupState.message}</Callout.Text>
        </Callout.Root>
      )}

      <Box style={{ flex: 1, overflow: "hidden", display: "flex", flexDirection: "column" }}>
        <Box className={desktopGrid} style={{ flex: 1 }}>
          <Box className={panelColumn}>
            <AgentHeader agent={captain} />
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
            <AgentHeader agent={mate} />
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
              <AgentHeader agent={captain} />
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
              <AgentHeader agent={mate} />
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

      <TaskBar sessionId={session.id} startupState={startupState} task={liveTask} />
    </Flex>
  );
}
