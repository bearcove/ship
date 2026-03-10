import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { Box, Flex, IconButton, Select, Spinner, Text, Tooltip } from "@radix-ui/themes";
import {
  Bug,
  CaretDown,
  CaretRight,
  FolderSimplePlus,
  Note,
  SpeakerHigh,
  SpeakerSlash,
} from "@phosphor-icons/react";
import type { AgentKind, ProjectInfo, SessionSummary, TaskStatus } from "../generated/ship";
import { useSoundEnabled } from "../context/SoundContext";
import { useAgentDiscovery } from "../hooks/useAgentDiscovery";
import { useAgentKindPrefs } from "../hooks/useAgentKindPrefs";
import { refreshSessionList } from "../hooks/useSessionList";
import { AddProjectDialog } from "../pages/SessionListPage";
import { getShipClient } from "../api/client";
import {
  projectActions,
  projectName,
  projectRow,
  sessionRow,
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
        <Text size="1" color="gray">
          {label}
        </Text>
      </Box>
      <Select.Root
        size="1"
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
  const navigate = useNavigate();

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
      navigate(`/sessions/${result.session_id}`);
    } finally {
      setCreating(false);
    }
  }

  return (
    <Box>
      <div className={projectRow} onClick={toggleCollapsed}>
        {collapsed ? (
          <CaretRight size={12} style={{ color: "var(--gray-9)", flexShrink: 0 }} />
        ) : (
          <CaretDown size={12} style={{ color: "var(--gray-9)", flexShrink: 0 }} />
        )}
        <Text className={projectName}>{project.name}</Text>
        <div className={projectActions}>
          <Tooltip content={`New session in ${project.name}`}>
            <IconButton
              size="1"
              variant="ghost"
              color="gray"
              aria-label={`New session in ${project.name}`}
              onClick={handleCreate}
              disabled={creating}
            >
              {creating ? <Spinner size="1" /> : <Note size={13} />}
            </IconButton>
          </Tooltip>
        </div>
      </div>

      {!collapsed && (
        <Box>
          {sessions.length === 0 ? (
            <div className={sessionRowEmpty}>No sessions</div>
          ) : (
            sessions.map((session) => {
              const isActive = session.id === currentSessionId;
              const title = session.current_task_title ?? session.branch_name;
              return (
                <Link
                  key={session.id}
                  to={`/sessions/${session.id}`}
                  className={sessionRow}
                  data-active={isActive ? "true" : "false"}
                  aria-current={isActive ? "page" : undefined}
                >
                  <Text className={sessionRowTitle}>{title}</Text>
                  {session.task_status && (
                    <div
                      className={sidebarStatusDot}
                      style={{ background: STATUS_DOT_COLOR[session.task_status.tag] }}
                    />
                  )}
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

        <Flex align="center" gap="3" pt="3" pb="5" px="2" style={{ flexShrink: 0 }}>
          <Tooltip content="Add project">
            <IconButton
              variant="ghost"
              size="2"
              color="gray"
              aria-label="Add project"
              onClick={() => setAddProjectOpen(true)}
            >
              <FolderSimplePlus size={16} />
            </IconButton>
          </Tooltip>
          <IconButton
            variant="ghost"
            size="2"
            color={debugMode ? "amber" : "gray"}
            onClick={onToggleDebug}
            aria-label={debugMode ? "Disable debug mode" : "Enable debug mode"}
          >
            <Bug size={16} />
          </IconButton>
          <IconButton
            variant="ghost"
            size="2"
            color="gray"
            onClick={() => setSoundEnabled(!soundEnabled)}
            aria-label={soundEnabled ? "Mute sounds" : "Unmute sounds"}
          >
            {soundEnabled ? <SpeakerHigh size={16} /> : <SpeakerSlash size={16} />}
          </IconButton>
        </Flex>

        <AddProjectDialog open={addProjectOpen} onOpenChange={setAddProjectOpen} />
      </Box>
    </>
  );
}
