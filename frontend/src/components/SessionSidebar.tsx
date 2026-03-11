import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { Box, Flex, IconButton, Select, Spinner, Text, Tooltip } from "@radix-ui/themes";
import {
  Archive,
  BugIcon,
  FolderIcon,
  FolderOpenIcon,
  FolderSimplePlusIcon,
  NoteIcon,
  SpeakerHighIcon,
  SpeakerSlashIcon,
} from "@phosphor-icons/react";
import type { AgentKind, ProjectInfo, SessionSummary, TaskStatus } from "../generated/ship";
import { useSoundEnabled } from "../context/SoundContext";
import { useAgentDiscovery } from "../hooks/useAgentDiscovery";
import { useAgentKindPrefs } from "../hooks/useAgentKindPrefs";
import { refreshSessionList } from "../hooks/useSessionList";
import { AddProjectDialog, ArchiveSessionDialog } from "../pages/SessionListPage";
import { getShipClient, useClientLogs } from "../api/client";
import { QrCodeButton } from "./QrCodeButton";
import {
  projectActions,
  projectName,
  projectRow,
  sessionRow,
  sessionRowArchiveBtn,
  sessionRowEmpty,
  sessionRowTitle,
  sidebarBackdrop,
  sidebarRoot,
  sidebarScrollArea,
  sidebarStatusDot,
} from "../styles/session-sidebar.css";

const STATUS_DOT_COLOR: Record<TaskStatus["tag"], string> = {
  Assigned: "var(--gray-9)",
  Working: "var(--blue-9)",
  ReviewPending: "var(--amber-9)",
  SteerPending: "var(--orange-9)",
  Accepted: "var(--green-9)",
  Cancelled: "var(--red-9)",
};

function useProjectCollapsed(name: string): [boolean, () => void] {
  const key = `ship:project-collapsed:${name}`;
  const [collapsed, setCollapsed] = useState(() => localStorage.getItem(key) === "true");
  function toggle() {
    setCollapsed((v) => {
      const next = !v;
      if (next) {
        localStorage.setItem(key, "true");
      } else {
        localStorage.removeItem(key);
      }
      return next;
    });
  }
  return [collapsed, toggle];
}

async function pickBranch(projectName: string): Promise<string> {
  try {
    const client = await getShipClient();
    const branches = await client.listBranches(projectName);
    return (
      branches.find((b) => b === "main") ??
      branches.find((b) => b === "master") ??
      branches[0] ??
      "main"
    );
  } catch {
    return "main";
  }
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

function ProjectGroup({
  project,
  sessions,
  currentSessionId,
  captainKind,
  mateKind,
}: {
  project: ProjectInfo;
  sessions: SessionSummary[];
  currentSessionId?: string;
  captainKind: AgentKind;
  mateKind: AgentKind;
}) {
  const [collapsed, toggleCollapsed] = useProjectCollapsed(project.name);
  const [creating, setCreating] = useState(false);
  const [archivingId, setArchivingId] = useState<string | null>(null);
  const [archiveConfirm, setArchiveConfirm] = useState<{
    session: SessionSummary;
    unmergedCommits: string[];
  } | null>(null);
  const navigate = useNavigate();

  async function handleArchive(session: SessionSummary, force: boolean) {
    setArchivingId(session.id);
    try {
      const client = await getShipClient();
      const result = await client.archiveSession({ id: session.id, force });
      if (result.tag === "Archived") {
        setArchiveConfirm(null);
        await refreshSessionList();
        if (currentSessionId === session.id) {
          navigate("/");
        }
      } else if (result.tag === "RequiresConfirmation") {
        setArchiveConfirm({ session, unmergedCommits: result.unmerged_commits });
      }
    } finally {
      setArchivingId(null);
    }
  }

  async function handleCreate(e: React.MouseEvent) {
    e.stopPropagation();
    if (creating) return;
    setCreating(true);
    try {
      const branch = await pickBranch(project.name);
      const client = await getShipClient();
      const result = await client.createSession({
        project: project.name,
        captain_kind: captainKind,
        mate_kind: mateKind,
        base_branch: branch,
        mcp_servers: null,
      });
      if (result.tag === "Failed") {
        // TODO: surface this better
        console.error("Failed to create session:", result.message);
        return;
      }
      await refreshSessionList();
      navigate(`/sessions/${result.slug}`);
    } finally {
      setCreating(false);
    }
  }

  return (
    <Box>
      <div className={projectRow} onClick={toggleCollapsed}>
        {collapsed ? (
          <FolderIcon size={18} style={{ color: "var(--gray-9)", flexShrink: 0 }} />
        ) : (
          <FolderOpenIcon size={18} style={{ color: "var(--gray-9)", flexShrink: 0 }} />
        )}
        <Text size="2" className={projectName}>
          {project.name}
        </Text>
        <div className={projectActions}>
          <Tooltip content={`New session in ${project.name}`}>
            <IconButton
              size="2"
              variant="ghost"
              color="gray"
              aria-label={`New session in ${project.name}`}
              onClick={handleCreate}
              disabled={creating}
            >
              {creating ? <Spinner size="2" /> : <NoteIcon size={13} />}
            </IconButton>
          </Tooltip>
        </div>
      </div>

      {archiveConfirm && (
        <ArchiveSessionDialog
          session={archiveConfirm.session}
          unmergedCommits={archiveConfirm.unmergedCommits}
          onConfirm={() => handleArchive(archiveConfirm.session, true)}
          onCancel={() => setArchiveConfirm(null)}
          archiving={archivingId === archiveConfirm.session.id}
        />
      )}

      {!collapsed && (
        <Box>
          {sessions.length === 0 ? (
            <div className={sessionRowEmpty}>No sessions</div>
          ) : (
            sessions.map((session) => {
              const isActive = session.id === currentSessionId;
              const title = session.title ?? session.current_task_title ?? session.branch_name;
              return (
                <Link
                  key={session.id}
                  to={`/sessions/${session.slug}`}
                  className={sessionRow}
                  data-active={isActive ? "true" : "false"}
                  aria-current={isActive ? "page" : undefined}
                >
                  <Text size="2" className={sessionRowTitle}>
                    {title}
                  </Text>
                  {session.task_status && (
                    <div
                      className={sidebarStatusDot}
                      style={{ background: STATUS_DOT_COLOR[session.task_status.tag] }}
                    />
                  )}
                  {/* r[proto.archive-session] */}
                  <Tooltip content="Archive session">
                    <IconButton
                      size="1"
                      variant="ghost"
                      color="gray"
                      className={sessionRowArchiveBtn}
                      loading={archivingId === session.id}
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        handleArchive(session, false);
                      }}
                    >
                      <Archive size={12} />
                    </IconButton>
                  </Tooltip>
                </Link>
              );
            })
          )}
        </Box>
      )}
    </Box>
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
  projects,
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

  const validProjects = projects.filter((p) => p.valid);

  return (
    <>
      {isOpen && <div className={sidebarBackdrop} onClick={onClose} />}
      <Box className={sidebarRoot} data-open={isOpen ? "true" : undefined}>
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
          {validProjects.map((project) => (
            <ProjectGroup
              key={project.name}
              project={project}
              sessions={sessions.filter((s) => s.project === project.name)}
              currentSessionId={currentSessionId}
              captainKind={captainKind}
              mateKind={mateKind}
            />
          ))}
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
