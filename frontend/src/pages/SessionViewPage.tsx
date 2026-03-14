import { useCallback, useEffect, useState } from "react";
import { useSwipeable } from "react-swipeable";
import { useParams, useNavigate } from "react-router-dom";
import { Box, Button, Callout, Flex, Spinner, Text } from "@radix-ui/themes";
import { Warning } from "@phosphor-icons/react";
import { useSession } from "../hooks/useSession";
import { useSessionState } from "../hooks/useSessionState";
import { refreshSessionList } from "../hooks/useSessionList";
import { UnifiedFeed } from "../components/UnifiedFeed";
import { UnifiedComposer } from "../components/UnifiedComposer";
import { SessionHeader } from "../components/SessionHeader";
import { SteerReview } from "../components/SteerReview";
import { HumanReview } from "../components/HumanReview";
import { SessionDebugPanel } from "../components/SessionDebugPanel";
import {
  agentRail,
  feedContentColumn,
  sessionFeedColumn,
  sessionViewRoot,
  slideInFromLeft,
  slideInFromRight,
} from "../styles/session-view.css";
import { AgentHeader } from "../components/AgentHeader";
import captainAvatar from "../assets/avatars/captain.png";
import mateAvatar from "../assets/avatars/mate.png";
import { getShipClient } from "../api/client";
import { ArchiveSessionDialog, NewSessionDialog } from "./SessionListPage";
import type { SessionSummary } from "../generated/ship";
import { useWorktreeDiffStats } from "../hooks/useWorktreeDiffStats";

// r[view.session]
// r[ui.layout.session-view]
// r[proto.hydration-flow]
export function SessionViewPage({ debugMode, allSessions }: { debugMode: boolean; allSessions: SessionSummary[]; onOpenSidebar?: () => void }) {
  const { sessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  const [archiving, setArchiving] = useState(false);
  const [archiveConfirm, setArchiveConfirm] = useState<string[] | null>(null);
  const [duplicateOpen, setDuplicateOpen] = useState(false);
  const [slideDirection, setSlideDirection] = useState<"left" | "right" | null>(null);

  const currentIndex = allSessions.findIndex((s) => s.slug === sessionId);
  const prevSession = currentIndex > 0 ? allSessions[currentIndex - 1] : null;
  const nextSession = currentIndex >= 0 && currentIndex < allSessions.length - 1 ? allSessions[currentIndex + 1] : null;

  const handleSwipe = useCallback(
    (direction: "left" | "right") => {
      if (direction === "right") {
        if (!prevSession) {
          navigate("/");
          return;
        }
      } else {
        if (!nextSession) return;
      }
      setSlideDirection(direction);
    },
    [nextSession, prevSession, navigate],
  );

  const swipeHandlers = useSwipeable({
    onSwipedLeft: () => handleSwipe("left"),
    onSwipedRight: () => handleSwipe("right"),
    delta: 72,
    preventScrollOnSwipe: true,
    trackTouch: true,
    trackMouse: false,
  });

  // r[event.client.hydration-sequence]: Step 1 — structural state
  const { session, error } = useSession(sessionId ?? "");
  // r[event.client.hydration-sequence]: Step 2/3 — event subscription + replay
  const eventState = useSessionState(sessionId ?? "", session);

  const diffStats = useWorktreeDiffStats(sessionId ?? "");

  // r[ui.keys.nav]
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key === "d" && e.metaKey) {
        e.preventDefault();
        setDuplicateOpen(true);
      }
      if (e.metaKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
        e.preventDefault();
        if (allSessions.length === 0) return;
        const idx = allSessions.findIndex((s) => s.slug === sessionId);
        let next: number;
        if (e.key === "ArrowUp") {
          next = idx <= 0 ? allSessions.length - 1 : idx - 1;
        } else {
          next = idx >= allSessions.length - 1 ? 0 : idx + 1;
        }
        navigate(`/sessions/${allSessions[next].slug}`);
      }
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [navigate, allSessions, sessionId]);

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
  const pendingHumanReview = eventState.pendingHumanReview;
  const isReplaying = eventState.phase !== "live";
  const replayLabel = eventState.connected
    ? eventState.replayEventCount > 0
      ? `Replaying ${eventState.replayEventCount} event${eventState.replayEventCount === 1 ? "" : "s"}…`
      : "Connected — waiting for replay…"
    : "Waiting for reconnect…";
  const taskCompletedDuration =
    eventState.currentTaskStartedAt && eventState.currentTaskCompletedAt
      ? Math.round(
        (new Date(eventState.currentTaskCompletedAt).getTime() -
          new Date(eventState.currentTaskStartedAt).getTime()) /
        1000,
      )
      : null;

  const liveTask =
    eventState.currentTaskId &&
      eventState.currentTaskTitle &&
      eventState.currentTaskDescription &&
      eventState.currentTaskStatus
      ? {
        id: eventState.currentTaskId,
        title: eventState.currentTaskTitle,
        description: eventState.currentTaskDescription,
        status: eventState.currentTaskStatus,
        assigned_at: eventState.currentTaskStartedAt,
        completed_at: eventState.currentTaskCompletedAt,
        steps: eventState.currentTaskSteps,
      }
      : session.current_task
        ? {
          ...session.current_task,
          steps:
            (
              session.current_task as unknown as {
                steps?: import("../generated/ship").PlanStep[];
              }
            ).steps ?? [],
        }
        : null;
  const matePlan = mate?.state.tag === "Working" ? (mate.state.plan ?? null) : null;
  const sessionDetail = session;
  const acceptedHistoryCount = sessionDetail.task_history.filter(
    (task) => task.status.tag === "Accepted",
  ).length;
  const hasAcceptedSessionWork = acceptedHistoryCount > 0 || liveTask?.status.tag === "Accepted";
  const tasksDone = acceptedHistoryCount + (liveTask?.status.tag === "Accepted" ? 1 : 0);
  const tasksTotal = sessionDetail.task_history.length + (liveTask ? 1 : 0);
  const planSteps = liveTask?.steps ?? [];
  const archiveSessionSummary: SessionSummary = {
    id: session.id,
    slug: session.slug,
    project: session.project,
    branch_name: session.branch_name,
    title: eventState.title ?? session.title,
    captain,
    mate,
    startup_state: startupState,
    current_task_title: liveTask?.title ?? null,
    current_task_description: liveTask?.description ?? null,
    task_status: liveTask?.status ?? null,
    autonomy_mode: session.autonomy_mode,
    created_at: session.created_at,
    diff_stats: diffStats,
    tasks_done: tasksDone,
    tasks_total: tasksTotal,
  };

  async function handleArchive(force: boolean) {
    setArchiving(true);
    try {
      const client = await getShipClient();
      const result = await client.archiveSession({ id: sessionDetail.id, force });
      if (result.tag === "Archived") {
        setArchiveConfirm(null);
        await refreshSessionList();
        navigate("/");
      } else if (result.tag === "RequiresConfirmation") {
        setArchiveConfirm(result.unmerged_commits);
      }
    } finally {
      setArchiving(false);
    }
  }

  return (
    <>
      {archiveConfirm && (
        <ArchiveSessionDialog
          session={archiveSessionSummary}
          unmergedCommits={archiveConfirm}
          onConfirm={() => void handleArchive(true)}
          onCancel={() => setArchiveConfirm(null)}
          archiving={archiving}
        />
      )}
      <NewSessionDialog
        open={duplicateOpen}
        onOpenChange={setDuplicateOpen}
        preselectedProject={session.project}
        preselectedCaptainKind={session.captain.kind}
        preselectedMateKind={session.mate.kind}
      />
      <Flex
        {...swipeHandlers}
        className={[
          sessionViewRoot,
          slideDirection === "right" ? slideInFromLeft : "",
          slideDirection === "left" ? slideInFromRight : "",
        ]
          .filter(Boolean)
          .join(" ")}
        onAnimationEnd={(event) => {
          if (event.target !== event.currentTarget || slideDirection === null) return;
          const target = slideDirection === "right" ? prevSession : nextSession;
          setSlideDirection(null);
          if (target) navigate(`/sessions/${target.slug}`);
        }}
      >
        <Flex style={{ flex: 1, overflow: "hidden", minHeight: 0 }}>
          <Box className={sessionFeedColumn}>
            <SessionHeader
              sessionId={session.id}
              project={session.project}
              title={eventState.title ?? session.title}
              branchName={session.branch_name}
              captain={captain ?? null}
              mate={mate ?? null}
              liveTask={liveTask}
              taskHistory={session.task_history}
              planSteps={planSteps}
              matePlan={matePlan}
              diffStats={diffStats}
              shouldRecommendArchiveSession={hasAcceptedSessionWork}
              onArchive={() => void handleArchive(false)}
              archiving={archiving}
            />
            <UnifiedFeed
              sessionId={session.id}
              captain={captain}
              mate={mate}
              blocks={eventState.unifiedBlocks.blocks}
              startupState={startupState}
              taskCompletedDuration={taskCompletedDuration}
              userAvatarUrl={session.user_avatar_url}
              loading={isReplaying}
              loadingLabel={replayLabel}
              debugMode={debugMode}
            />
            {debugMode && (
              <SessionDebugPanel
                captainAcpInfo={eventState.captainAcpInfo}
                mateAcpInfo={eventState.mateAcpInfo}
              />
            )}
            {hasAcceptedSessionWork &&
              (liveTask === null || liveTask.status.tag === "Accepted") && (
                <Flex justify="center" px="4" pb="2">
                  <Button size="3" variant="outline" onClick={() => void handleArchive(false)}>
                    Archive session
                  </Button>
                </Flex>
              )}
            <Box className={feedContentColumn}>
              <UnifiedComposer
                sessionId={session.id}
                captain={captain}
                mate={mate}
                startupState={startupState}
                taskStatus={liveTask?.status ?? null}
              />
            </Box>
          </Box>
        </Flex>

        {session.pending_steer && (
          <SteerReview
            sessionId={session.id}
            steerText={session.pending_steer}
            onDismiss={() => { }}
          />
        )}
        {pendingHumanReview && <HumanReview sessionId={session.id} review={pendingHumanReview} />}
      </Flex>
    </>
  );
}

export function SessionAgentRail({ sessionId }: { sessionId: string }) {
  // Don't call useSession here — SessionViewPage already hydrates the shared
  // subscription for this sessionId. A second independent useSession call
  // produces a different object reference for the same data, which makes
  // useSessionState's `session !== sub.lastHydratedSession` check ping-pong
  // infinitely between the two objects.
  const eventState = useSessionState(sessionId, null);

  const captain = eventState.captain;
  const mate = eventState.mate;

  if (!(captain ?? mate)) return null;

  return (
    <Box className={agentRail}>
      {captain && <AgentHeader sessionId={sessionId} agent={captain} avatarSrc={captainAvatar} />}
      {mate && <AgentHeader sessionId={sessionId} agent={mate} avatarSrc={mateAvatar} />}
    </Box>
  );
}
