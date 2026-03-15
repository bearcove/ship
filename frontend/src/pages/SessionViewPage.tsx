import TurndownService from "turndown";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Box, Callout, Flex, Spinner, Text } from "@radix-ui/themes";
import { Warning } from "@phosphor-icons/react";
import { useSession } from "../hooks/useSession";
import { useSessionState } from "../hooks/useSessionState";
import { refreshSessionList } from "../hooks/useSessionList";
import { useDocumentDrop } from "../hooks/useDocumentDrop";
import { UnifiedFeed } from "../components/UnifiedFeed";
import { UnifiedComposer, type UnifiedComposerHandle } from "../components/UnifiedComposer";
import { SessionHeader } from "../components/SessionHeader";
import { SteerReview } from "../components/SteerReview";
import { HumanReview } from "../components/HumanReview";

import { SessionDebugPanel } from "../components/SessionDebugPanel";
import {
  agentRail,
  feedContentColumn,
  pageDot,
  pageDotActive,
  pageDotsRow,
  sessionFeedColumn,
  sessionViewRoot,
} from "../styles/session-view.css";
import { AgentHeader } from "../components/AgentHeader";
import captainAvatar from "../assets/avatars/captain.png";
import mateAvatar from "../assets/avatars/mate.png";
import { getShipClient } from "../api/client";
import { ArchiveSessionDialog } from "../components/ArchiveSessionDialog";
import { NewSessionDialog } from "../components/NewSessionDialog";
import type { SessionSummary } from "../generated/ship";
import { useWorktreeDiffStats } from "../hooks/useWorktreeDiffStats";
import { sortSessions } from "./session-list-utils";

const turndown = new TurndownService();

// r[view.session]
// r[ui.layout.session-view]
// r[proto.hydration-flow]
export function SessionViewPage({
  sessionId,
  isActive,
  debugMode,
  allSessions = [],
  onArchived,
}: {
  sessionId: string;
  isActive: boolean;
  debugMode: boolean;
  allSessions?: SessionSummary[];
  onArchived?: () => void;
}) {
  const navigate = useNavigate();
  const composerRef = useRef<UnifiedComposerHandle>(null);
  const feedColumnRef = useRef<HTMLDivElement>(null);
  const [archiving, setArchiving] = useState(false);
  const [archiveConfirm, setArchiveConfirm] = useState<string[] | null>(null);
  const [duplicateOpen, setDuplicateOpen] = useState(false);

  const orderedSessions = useMemo(() => sortSessions(allSessions), [allSessions]);
  const { currentIndex, hasSessionCycle, prevSession, nextSession } = useMemo(() => {
    const nextIndex = orderedSessions.findIndex((s) => s.slug === sessionId);
    const canCycle = nextIndex >= 0 && orderedSessions.length > 0;
    return {
      currentIndex: nextIndex,
      hasSessionCycle: canCycle,
      prevSession: canCycle
        ? orderedSessions[(nextIndex - 1 + orderedSessions.length) % orderedSessions.length]
        : null,
      nextSession: canCycle ? orderedSessions[(nextIndex + 1) % orderedSessions.length] : null,
    };
  }, [orderedSessions, sessionId]);

  // Physical swipe tracking refs
  const sessionViewRef = useRef<HTMLDivElement>(null);
  const touchStartX = useRef(0);
  const touchStartY = useRef(0);
  const isDragging = useRef(false);
  const isVerticalGesture = useRef(false);
  const currentDeltaX = useRef(0);

  const onTouchStart = useCallback((e: React.TouchEvent) => {
    if (window.innerWidth > 700) return;
    const touch = e.touches[0];
    touchStartX.current = touch.clientX;
    touchStartY.current = touch.clientY;
    isDragging.current = false;
    isVerticalGesture.current = false;
    currentDeltaX.current = 0;
  }, []);

  const onTouchMove = useCallback((e: React.TouchEvent) => {
    if (window.innerWidth > 700 || isVerticalGesture.current) return;
    const touch = e.touches[0];
    const deltaX = touch.clientX - touchStartX.current;
    const deltaY = touch.clientY - touchStartY.current;

    if (!isDragging.current) {
      // Decide direction on first significant movement
      if (Math.abs(deltaX) < 8 && Math.abs(deltaY) < 8) return;
      if (Math.abs(deltaY) > Math.abs(deltaX)) {
        isVerticalGesture.current = true;
        return;
      }
      isDragging.current = true;
    }

    e.preventDefault();
    currentDeltaX.current = deltaX;
    const el = sessionViewRef.current;
    if (el) {
      el.style.transition = "none";
      el.style.transform = `translateX(${deltaX}px)`;
      el.style.willChange = "transform";
    }
  }, []);

  const onTouchEnd = useCallback(() => {
    if (window.innerWidth > 700 || !isDragging.current) return;
    isDragging.current = false;
    const el = sessionViewRef.current;
    if (!el) return;

    const threshold = window.innerWidth * 0.3;
    const deltaX = currentDeltaX.current;

    if (Math.abs(deltaX) > threshold) {
      // Swipe completed — animate off-screen then navigate
      const target = deltaX > 0 ? prevSession : nextSession;
      if (target) {
        const direction = deltaX > 0 ? 1 : -1;
        el.style.transition = "transform 200ms ease-out";
        el.style.transform = `translateX(${direction * window.innerWidth}px)`;
        const handleEnd = () => {
          el.removeEventListener("transitionend", handleEnd);
          el.style.transition = "";
          el.style.transform = "";
          el.style.willChange = "";
          navigate(`/sessions/${target.slug}`);
        };
        el.addEventListener("transitionend", handleEnd, { once: true });
        return;
      }
    }

    // Snap back
    el.style.transition = "transform 200ms ease-out";
    el.style.transform = "translateX(0)";
    const resetStyle = () => {
      el.removeEventListener("transitionend", resetStyle);
      el.style.transition = "";
      el.style.transform = "";
      el.style.willChange = "";
    };
    el.addEventListener("transitionend", resetStyle, { once: true });
  }, [navigate, prevSession, nextSession]);

  // r[event.client.hydration-sequence]: Step 1 — structural state
  const { session, error } = useSession(sessionId);
  // r[event.client.hydration-sequence]: Step 2/3 — event subscription + replay
  const eventState = useSessionState(sessionId, session);

  const diffStats = useWorktreeDiffStats(sessionId);

  // r[proto.mark-session-read]
  useEffect(() => {
    if (!session) return;
    getShipClient().then((client) => client.markSessionRead(sessionId));
  }, [sessionId, !!session]);

  const handleFeedImageDrop = useCallback((files: File[]) => {
    composerRef.current?.addImageFiles(files);
    composerRef.current?.focusComposer();
  }, []);

  const isImageDragOver = useDocumentDrop(feedColumnRef.current, handleFeedImageDrop);

  useEffect(() => {
    composerRef.current?.setDragOver(isImageDragOver);
  }, [isImageDragOver]);

  function isEditableTarget(target: EventTarget | null): boolean {
    return (
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      (target instanceof HTMLElement && target.isContentEditable)
    );
  }

  // r[ui.keys.nav]
  useEffect(() => {
    if (!isActive) return;
    function handler(e: KeyboardEvent) {
      if (isEditableTarget(e.target)) return;
      if (e.key === "d" && e.metaKey) {
        e.preventDefault();
        setDuplicateOpen(true);
        return;
      }
      if (e.metaKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
        e.preventDefault();
        if (!hasSessionCycle) return;
        let next: number;
        if (e.key === "ArrowUp") {
          next = currentIndex <= 0 ? orderedSessions.length - 1 : currentIndex - 1;
        } else {
          next = currentIndex >= orderedSessions.length - 1 ? 0 : currentIndex + 1;
        }
        navigate(`/sessions/${orderedSessions[next].slug}`);
        return;
      }
      if (!e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "r") {
        const sel = window.getSelection();
        if (!sel || sel.isCollapsed || sel.rangeCount === 0) return;
        const fragment = sel.getRangeAt(0).cloneContents();
        const div = document.createElement("div");
        div.appendChild(fragment);
        const html = div.innerHTML;
        if (!html.trim()) return;
        e.preventDefault();
        const markdown = turndown.turndown(html);
        composerRef.current?.insertQuote(markdown);
        sel.removeAllRanges();
        return;
      }
      if (!e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "c") {
        e.preventDefault();
        composerRef.current?.focusComposer();
      }
    }

    function copyHandler(e: ClipboardEvent) {
      if (isEditableTarget(e.target)) return;
      const sel = window.getSelection();
      if (!sel || sel.isCollapsed || sel.rangeCount === 0) return;
      const fragment = sel.getRangeAt(0).cloneContents();
      const div = document.createElement("div");
      div.appendChild(fragment);
      const html = div.innerHTML;
      if (!html.trim()) return;
      const markdown = turndown.turndown(html);
      e.clipboardData?.setData("text/plain", markdown);
      e.preventDefault();
    }

    window.addEventListener("keydown", handler);
    document.addEventListener("copy", copyHandler);
    return () => {
      window.removeEventListener("keydown", handler);
      document.removeEventListener("copy", copyHandler);
    };
  }, [currentIndex, hasSessionCycle, isActive, navigate, orderedSessions]);

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
            onClick={() => navigate("/sessions/admiral")}
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
    is_admiral: false,
    is_read: true,
  };

  async function handleArchive(force: boolean) {
    setArchiving(true);
    try {
      const client = await getShipClient();
      const result = await client.archiveSession({ id: sessionDetail.id, force });
      if (result.tag === "Archived") {
        setArchiveConfirm(null);
        onArchived?.();
        await refreshSessionList();
        navigate("/sessions/admiral");
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
        className={sessionViewRoot}
      >
        <Flex style={{ flex: 1, overflow: "hidden", minHeight: 0 }}>
          <Box className={sessionFeedColumn} ref={feedColumnRef}>
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
              checksState={eventState.checksState}
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
              captainTurnStartedAt={eventState.captainTurnStartedAt}
              mateTurnStartedAt={eventState.mateTurnStartedAt}
              userAvatarUrl={session.user_avatar_url}
              loading={isReplaying}
              loadingLabel={replayLabel}
              debugMode={debugMode}
              onArchive={
                hasAcceptedSessionWork && (liveTask === null || liveTask.status.tag === "Accepted")
                  ? () => void handleArchive(false)
                  : undefined
              }
            />
            {debugMode && (
              <SessionDebugPanel
                captainAcpInfo={eventState.captainAcpInfo}
                mateAcpInfo={eventState.mateAcpInfo}
              />
            )}
          </Box>
        </Flex>
        <div
          ref={sessionViewRef}
          onTouchStart={onTouchStart}
          onTouchMove={onTouchMove}
          onTouchEnd={onTouchEnd}
        >
          <Box className={feedContentColumn}>
            <UnifiedComposer
              ref={composerRef}
              sessionId={session.id}
              captain={captain}
              mate={mate}
              startupState={startupState}
              taskStatus={liveTask?.status ?? null}
              captainBlocks={eventState.captainBlocks}
            />
          </Box>
          {orderedSessions.length > 1 && (
            <div className={pageDotsRow}>
              {orderedSessions.map((s, i) => (
                <div
                  key={s.slug}
                  className={`${pageDot} ${i === currentIndex ? pageDotActive : ""}`}
                  onClick={() => navigate(`/sessions/${s.slug}`)}
                />
              ))}
            </div>
          )}
        </div>

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
