import { useEffect, useState } from "react";
import { useParams, useNavigate, Link } from "react-router-dom";
import { Box, Callout, Flex, IconButton, Spinner, Text, Tooltip } from "@radix-ui/themes";
import { Archive, ArrowLeft, List, Plus, Warning } from "@phosphor-icons/react";
import { useSession } from "../hooks/useSession";
import { useSessionState } from "../hooks/useSessionState";
import { refreshSessionList } from "../hooks/useSessionList";
import { useWorktreeDiffStats } from "../hooks/useWorktreeDiffStats";
import { UnifiedFeed } from "../components/UnifiedFeed";
import { UnifiedComposer } from "../components/UnifiedComposer";
import { SessionTaskDrawer } from "../components/SessionTaskDrawer";
import { PlanPanel } from "../components/PlanPanel";
import { SteerReview } from "../components/SteerReview";
import { HumanReview } from "../components/HumanReview";
import {
  agentRail,
  hamburgerBtn,
  sessionFeedColumn,
  sessionTopBar,
  sessionTopBarActions,
  sessionTopBarAgentSection,
  sessionTopBarBreadcrumb,
  sessionTopBarDivider,
  sessionTopBarLeft,
  sessionTopBarRight,
  sessionViewRoot,
} from "../styles/session-view.css";
import { AgentHeader } from "../components/AgentHeader";
import { AgentModelPicker } from "../components/AgentModelPicker";
import { AgentEffortPicker } from "../components/AgentEffortPicker";
import captainAvatar from "../assets/avatars/captain.png";
import mateAvatar from "../assets/avatars/mate.png";
import { getShipClient } from "../api/client";
import { ArchiveSessionDialog, NewSessionDialog } from "./SessionListPage";
import type { AgentSnapshot, SessionSummary, TaskRecord } from "../generated/ship";

function SessionTopBar({
  sessionId,
  project,
  title,
  branchName,
  captain,
  mate,
  onOpenSidebar,
  onArchive,
  archiving,
}: {
  sessionId: string;
  project: string;
  title: string | null;
  branchName: string;
  captain: AgentSnapshot | null;
  mate: AgentSnapshot | null;
  onOpenSidebar: () => void;
  onArchive: () => void;
  archiving: boolean;
}) {
  const displayTitle = title ?? branchName;
  const [newSessionOpen, setNewSessionOpen] = useState(false);
  return (
    <>
      <div className={sessionTopBar}>
        <div className={sessionTopBarLeft}>
          <IconButton
            className={hamburgerBtn}
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
          <div className={sessionTopBarBreadcrumb}>
            <Text size="2" color="gray">
              {project}
            </Text>
            <Text size="2" color="gray">
              {" / "}
            </Text>
            <Text size="2" color="gray">
              {displayTitle}
            </Text>
          </div>
        </div>
        <div className={sessionTopBarRight}>
          {captain && (
            <div className={sessionTopBarAgentSection}>
              <AgentModelPicker sessionId={sessionId} agent={captain} />
              <AgentEffortPicker sessionId={sessionId} agent={captain} />
            </div>
          )}
          {captain && mate && <div className={sessionTopBarDivider} />}
          {mate && (
            <div className={sessionTopBarAgentSection}>
              <AgentModelPicker sessionId={sessionId} agent={mate} />
              <AgentEffortPicker sessionId={sessionId} agent={mate} />
            </div>
          )}
        </div>
        <div className={sessionTopBarActions}>
          <Tooltip content="New session">
            <IconButton
              variant="ghost"
              color="gray"
              size="2"
              onClick={() => setNewSessionOpen(true)}
              aria-label="New session"
            >
              <Plus size={16} />
            </IconButton>
          </Tooltip>
          <Tooltip content="Archive session">
            <IconButton
              variant="ghost"
              color="gray"
              size="2"
              onClick={onArchive}
              aria-label="Archive session"
              loading={archiving}
            >
              <Archive size={16} />
            </IconButton>
          </Tooltip>
        </div>
      </div>
      <NewSessionDialog
        open={newSessionOpen}
        onOpenChange={setNewSessionOpen}
        preselectedProject={project}
      />
    </>
  );
}

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
  const [archiving, setArchiving] = useState(false);
  const [archiveConfirm, setArchiveConfirm] = useState<string[] | null>(null);
  const [duplicateOpen, setDuplicateOpen] = useState(false);
  // r[event.client.hydration-sequence]: Step 1 — structural state
  const { session, error } = useSession(sessionId ?? "");
  // r[event.client.hydration-sequence]: Step 2/3 — event subscription + replay
  const eventState = useSessionState(sessionId ?? "", session);

  // r[ui.keys.nav]
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key === "Escape") onOpenSidebar();
      if (e.key === "d" && e.metaKey) {
        e.preventDefault();
        setDuplicateOpen(true);
      }
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
  const pendingHumanReview = eventState.pendingHumanReview;
  const diffStats = useWorktreeDiffStats(session.id);
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
          assigned_at: eventState.currentTaskStartedAt,
          completed_at: eventState.currentTaskCompletedAt,
        }
      : session.current_task;
  const matePlan = mate?.state.tag === "Working" ? (mate.state.plan ?? null) : null;
  const sessionDetail = session;
  const tasksDone =
    sessionDetail.task_history.filter((task) => task.status.tag === "Accepted").length +
    (liveTask?.status.tag === "Accepted" ? 1 : 0);
  const tasksTotal = sessionDetail.task_history.length + (liveTask ? 1 : 0);
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
    diff_stats: null,
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
      <Flex className={sessionViewRoot}>
        <Flex style={{ flex: 1, overflow: "hidden", minHeight: 0 }}>
          <Box className={sessionFeedColumn}>
            <SessionTopBar
              sessionId={session.id}
              project={session.project}
              title={eventState.title ?? session.title}
              branchName={session.branch_name}
              captain={captain ?? null}
              mate={mate ?? null}
              onOpenSidebar={onOpenSidebar}
              onArchive={() => void handleArchive(false)}
              archiving={archiving}
            />
            <SessionTaskDrawer
              liveTask={liveTask}
              taskHistory={session.task_history}
              branchName={session.branch_name}
              diffStats={diffStats}
              tasksDone={tasksDone}
              tasksTotal={tasksTotal}
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
            {matePlan && matePlan.length > 0 && <PlanPanel steps={matePlan} />}
            <UnifiedComposer
              sessionId={session.id}
              captain={captain}
              mate={mate}
              startupState={startupState}
              taskStatus={liveTask?.status ?? null}
            />
          </Box>
        </Flex>

        {session.pending_steer && (
          <SteerReview
            sessionId={session.id}
            steerText={session.pending_steer}
            onDismiss={() => {}}
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
