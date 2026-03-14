import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { Box, Flex, IconButton, Text, Tooltip } from "@radix-ui/themes";
import {
  BugIcon,
  FolderSimplePlusIcon,
  NotePencilIcon,
  SpeakerHighIcon,
  SpeakerSlashIcon,
} from "@phosphor-icons/react";
import type { SessionSummary, TaskStatus } from "../generated/ship";
import { useSoundEnabled } from "../context/SoundContext";
import { AddProjectDialog, NewSessionDialog } from "../pages/SessionListPage";
import { sortSessions } from "../pages/session-list-utils";
import { useClientLogs } from "../api/client";
import { QrCodeButton } from "./QrCodeButton";
import { SessionRecordingBadge } from "./SessionRecordingBadge";
import {
  sessionRow,
  sessionRowEmpty,
  sessionRowTitle,
  sidebarBackdrop,
  sidebarHomeLink,
  sidebarRoot,
  sidebarScrollArea,
} from "../styles/session-sidebar.css";

function statusLabel(status: TaskStatus | null): string {
  if (!status) return "Idle";
  switch (status.tag) {
    case "ReviewPending":
      return "Review";
    case "SteerPending":
      return "Steer";
    case "RebaseConflict":
      return "Conflict";
    case "Working":
      return "Working";
    case "Assigned":
      return "Starting";
    case "Accepted":
      return "Done";
    case "Cancelled":
      return "Cancelled";
  }
}

function SessionRow({
  session,
  currentSessionId,
  onClose,
}: {
  session: SessionSummary;
  currentSessionId?: string;
  onClose?: () => void;
}) {
  const isActive = session.slug === currentSessionId;
  const isActiveTask = ["Working", "Assigned", "ReviewPending", "SteerPending"].includes(
    session.task_status?.tag ?? "",
  );
  const hasTitle = isActiveTask ? !!session.current_task_title : !!session.title;
  const title = hasTitle
    ? (isActiveTask && session.current_task_title
      ? session.current_task_title
      : session.title!)
    : "Untitled";
  const diffStats = session.diff_stats;
  const showTaskCounts = session.tasks_total > 0;
  const showDiffStats =
    diffStats != null && (diffStats.lines_added > 0 || diffStats.lines_removed > 0);

  return (
    <Link
      to={`/sessions/${session.slug}`}
      className={sessionRow}
      data-active={isActive ? "true" : "false"}
      aria-current={isActive ? "page" : undefined}
      onClick={() => onClose?.()}
    >
      <Flex direction="column" gap="1" style={{ minWidth: 0, flex: 1 }}>
        <Flex align="center" gap="2" style={{ minWidth: 0 }}>
          <Text size="2" className={sessionRowTitle} color={hasTitle ? undefined : "gray"}>
            {title}
          </Text>
          <SessionRecordingBadge sessionId={session.id} compact />
        </Flex>
        <Text
          size="1"
          color="gray"
          style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
        >
          {session.project} · {statusLabel(session.task_status)}
          {showTaskCounts && (
            <span>
              {" "}
              · {session.tasks_done}/{session.tasks_total}
            </span>
          )}
          {showDiffStats && (
            <>
              <span> · </span>
              <span style={{ color: "var(--green-10)" }}>+{diffStats.lines_added}</span>
              <span> </span>
              <span style={{ color: "var(--red-10)" }}>-{diffStats.lines_removed}</span>
            </>
          )}
        </Text>
      </Flex>
    </Link>
  );
}

interface Props {
  sessions: SessionSummary[];
  currentSessionId?: string;
  debugMode: boolean;
  onToggleDebug: () => void;
  isOpen?: boolean;
  onClose?: () => void;
}

// r[ui.session-list.nav]
export function SessionSidebar({
  sessions,
  currentSessionId,
  debugMode,
  onToggleDebug,
  isOpen,
  onClose,
}: Props) {
  const [addProjectOpen, setAddProjectOpen] = useState(false);
  const [newSessionOpen, setNewSessionOpen] = useState(false);
  const { soundEnabled, setSoundEnabled } = useSoundEnabled();
  const clientLogs = useClientLogs();
  const sortedSessions = useMemo(() => sortSessions(sessions), [sessions]);
  const currentSession = useMemo(
    () => sessions.find((s) => s.slug === currentSessionId),
    [currentSessionId, sessions],
  );

  return (
    <>
      {isOpen && <div className={sidebarBackdrop} onClick={onClose} />}
      <Box className={sidebarRoot} data-open={isOpen ? "true" : undefined}>
        <div className={sidebarHomeLink}>
          <Link to="/" style={{ textDecoration: "none", color: "inherit", display: "flex", justifyContent: "center" }}>
            <img
              src="/ship-logo-w256.png"
              alt="Ship"
              style={{ width: 120, height: 120, objectFit: "contain", padding: 20 }}
            />
          </Link>
        </div>

        <Box className={sidebarScrollArea}>
          <Box px="3" pb="2">
            <button
              type="button"
              onClick={() => setNewSessionOpen(true)}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "var(--space-2)",
                padding: "var(--space-1) var(--space-1)",
                border: "none",
                borderRadius: "var(--radius-2)",
                background: "transparent",
                color: "var(--gray-11)",
                cursor: "pointer",
                fontSize: "var(--font-size-2)",
              }}
            >
              <NotePencilIcon size={16} />
              New session
            </button>
          </Box>
          {sessions.length === 0 ? (
            <div className={sessionRowEmpty}>No sessions</div>
          ) : (
            sortedSessions.map((session) => (
              <SessionRow
                key={session.id}
                session={session}
                currentSessionId={currentSessionId}
                onClose={onClose}
              />
            ))
          )}
        </Box>

        {debugMode && (
          <Box
            style={{
              flexShrink: 0,
              borderTop: "1px solid var(--gray-a4)",
              maxHeight: 240,
              overflowY: "auto",
              padding: "var(--space-2) var(--space-3)",
              display: "flex",
              flexDirection: "column",
              gap: 2,
            }}
          >
            {clientLogs.length === 0 ? (
              <Text size="1" color="gray">
                No connection events yet.
              </Text>
            ) : (
              clientLogs.map((entry, i) => (
                <Text
                  key={i}
                  size="1"
                  color={entry.level === "warn" ? "amber" : "gray"}
                  style={{ fontFamily: "monospace", wordBreak: "break-all" }}
                >
                  {new Date(entry.ts).toISOString().slice(11, 23)} {entry.message}
                  {Object.keys(entry.details).length > 0 ? " " + JSON.stringify(entry.details) : ""}
                </Text>
              ))
            )}
          </Box>
        )}

        <Flex align="center" gap="3" pt="3" pb="4" px="3" style={{ flexShrink: 0 }}>
          <Tooltip content="Add project">
            <IconButton
              variant="ghost"
              size="2"
              color="gray"
              aria-label="Add project"
              onClick={() => setAddProjectOpen(true)}
            >
              <FolderSimplePlusIcon size={16} />
            </IconButton>
          </Tooltip>
          <IconButton
            variant="ghost"
            size="2"
            color={debugMode ? "amber" : "gray"}
            onClick={onToggleDebug}
            aria-label={debugMode ? "Disable debug mode" : "Enable debug mode"}
          >
            <BugIcon size={16} />
          </IconButton>
          <IconButton
            variant="ghost"
            size="2"
            color="gray"
            onClick={() => setSoundEnabled(!soundEnabled)}
            aria-label={soundEnabled ? "Mute sounds" : "Unmute sounds"}
          >
            {soundEnabled ? <SpeakerHighIcon size={16} /> : <SpeakerSlashIcon size={16} />}
          </IconButton>
          <QrCodeButton />
        </Flex>

        <AddProjectDialog open={addProjectOpen} onOpenChange={setAddProjectOpen} />
        <NewSessionDialog
          open={newSessionOpen}
          onOpenChange={setNewSessionOpen}
          preselectedProject={currentSession?.project}
          preselectedCaptainKind={currentSession?.captain.kind}
          preselectedMateKind={currentSession?.mate.kind}
        />
      </Box>
    </>
  );
}
