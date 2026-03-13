import { useState } from "react";
import { Link } from "react-router-dom";
import { Box, Flex, IconButton, Select, Text, Tooltip } from "@radix-ui/themes";
import {
  BugIcon,
  FolderSimplePlusIcon,
  SpeakerHighIcon,
  SpeakerSlashIcon,
} from "@phosphor-icons/react";
import type { AgentKind, ProjectInfo, SessionSummary, TaskStatus } from "../generated/ship";
import { useSoundEnabled } from "../context/SoundContext";
import { useAgentDiscovery } from "../hooks/useAgentDiscovery";
import { useAgentKindPrefs } from "../hooks/useAgentKindPrefs";
import { AddProjectDialog } from "../pages/SessionListPage";
import { useClientLogs } from "../api/client";
import { QrCodeButton } from "./QrCodeButton";
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

function sortSessions(sessions: SessionSummary[]): SessionSummary[] {
  const priority = (session: SessionSummary) => {
    const tag = session.task_status?.tag;
    if (tag === "ReviewPending" || tag === "SteerPending") return 0;
    if (tag === "Working" || tag === "Assigned") return 1;
    return 2;
  };
  return [...sessions].sort((a, b) => priority(a) - priority(b));
}

function AgentKindSelect({
  label,
  value,
  onChange,
  claudeAvailable,
  codexAvailable,
}: {
  label: string;
  value: AgentKind;
  onChange: (k: AgentKind) => void;
  claudeAvailable: boolean;
  codexAvailable: boolean;
}) {
  return (
    <Flex align="center" gap="2">
      <Box width="7" flexShrink="0">
        <Text size="2" color="gray">
          {label}
        </Text>
      </Box>
      <Select.Root
        size="2"
        value={value.tag}
        onValueChange={(v) => onChange({ tag: v as "Claude" | "Codex" })}
      >
        <Select.Trigger variant="ghost" />
        <Select.Content>
          <Select.Item value="Claude" disabled={!claudeAvailable}>
            Claude
          </Select.Item>
          <Select.Item value="Codex" disabled={!codexAvailable}>
            Codex
          </Select.Item>
        </Select.Content>
      </Select.Root>
    </Flex>
  );
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
  const title = session.title ?? session.current_task_title ?? session.branch_name;
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
        <Text size="2" className={sessionRowTitle}>
          {title}
        </Text>
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
  projects: ProjectInfo[];
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
  const { soundEnabled, setSoundEnabled } = useSoundEnabled();
  const discovery = useAgentDiscovery();
  const { captainKind, setCaptainKind, mateKind, setMateKind } = useAgentKindPrefs();
  const clientLogs = useClientLogs();

  return (
    <>
      {isOpen && <div className={sidebarBackdrop} onClick={onClose} />}
      <Box className={sidebarRoot} data-open={isOpen ? "true" : undefined}>
        <div className={sidebarHomeLink}>
          <Link to="/" style={{ textDecoration: "none", color: "inherit" }}>
            <Text size="3" weight="bold">
              Ship
            </Text>
          </Link>
        </div>
        <Flex direction="column" gap="3" pt="3" pb="3" px="3">
          <AgentKindSelect
            label="Captain"
            value={captainKind}
            onChange={setCaptainKind}
            claudeAvailable={discovery.claude}
            codexAvailable={discovery.codex}
          />
          <AgentKindSelect
            label="Mate"
            value={mateKind}
            onChange={setMateKind}
            claudeAvailable={discovery.claude}
            codexAvailable={discovery.codex}
          />
        </Flex>

        <Box className={sidebarScrollArea}>
          {sessions.length === 0 ? (
            <div className={sessionRowEmpty}>No sessions</div>
          ) : (
            sortSessions(sessions).map((session) => (
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
      </Box>
    </>
  );
}
