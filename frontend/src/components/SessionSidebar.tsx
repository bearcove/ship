import { useState } from "react";
import { Link } from "react-router-dom";
import { Box, Flex, IconButton, Tooltip } from "@radix-ui/themes";
import { Bug, CaretLeft, CaretRight, Plus, SpeakerHigh, SpeakerSlash } from "@phosphor-icons/react";
import type { SessionSummary, TaskStatus } from "../generated/ship";
import { useSoundEnabled } from "../context/SoundContext";
import { NewSessionDialog } from "../pages/SessionListPage";
import {
  sidebarFooter,
  sidebarRoot,
  sidebarScrollArea,
  sidebarStatusDot,
  sidebarStatusRow,
  sidebarTab,
  sidebarTabDesc,
  sidebarTabProject,
} from "../styles/session-sidebar.css";

const STATUS_DOT_COLOR: Record<TaskStatus["tag"], string> = {
  Assigned: "var(--gray-9)",
  Working: "var(--blue-9)",
  ReviewPending: "var(--amber-9)",
  SteerPending: "var(--orange-9)",
  Accepted: "var(--green-9)",
  Cancelled: "var(--red-9)",
};

interface Props {
  sessions: SessionSummary[];
  currentSessionId?: string;
  currentProject?: string;
  debugMode: boolean;
  onToggleDebug: () => void;
}

function useCollapsed(): [boolean, (v: boolean) => void] {
  const [collapsed, setCollapsedState] = useState(
    () => localStorage.getItem("ship:sidebar-collapsed") === "true",
  );
  function setCollapsed(v: boolean) {
    setCollapsedState(v);
    localStorage.setItem("ship:sidebar-collapsed", String(v));
  }
  return [collapsed, setCollapsed];
}

// r[ui.session-list.nav]
export function SessionSidebar({
  sessions,
  currentSessionId,
  currentProject,
  debugMode,
  onToggleDebug,
}: Props) {
  const [newSessionOpen, setNewSessionOpen] = useState(false);
  const [collapsed, setCollapsed] = useCollapsed();
  const { soundEnabled, setSoundEnabled } = useSoundEnabled();

  return (
    <Box className={sidebarRoot} data-collapsed={collapsed}>
      {!collapsed && (
        <Box className={sidebarScrollArea}>
          {sessions.map((session) => {
            const isActive = session.id === currentSessionId;
            const rawTitle = session.current_task_title;
            const desc = rawTitle
              ? rawTitle.length > 50
                ? `${rawTitle.slice(0, 47)}…`
                : rawTitle
              : null;

            return (
              <Link
                key={session.id}
                to={`/sessions/${session.id}`}
                className={sidebarTab}
                data-active={isActive ? "true" : "false"}
                aria-current={isActive ? "page" : undefined}
              >
                <div className={sidebarTabProject}>{session.project}</div>
                {desc && <div className={sidebarTabDesc}>{desc}</div>}
                {session.task_status && (
                  <div className={sidebarStatusRow}>
                    <div
                      className={sidebarStatusDot}
                      style={{ background: STATUS_DOT_COLOR[session.task_status.tag] }}
                    />
                  </div>
                )}
              </Link>
            );
          })}
        </Box>
      )}

      {collapsed && <Box style={{ flex: 1 }} />}

      <Flex className={sidebarFooter} align="center" gap="1" wrap="wrap">
        {!collapsed && (
          <>
            <IconButton
              variant="ghost"
              size="2"
              aria-label="New session"
              onClick={() => setNewSessionOpen(true)}
            >
              <Plus size={16} />
            </IconButton>
            <IconButton
              variant={debugMode ? "solid" : "ghost"}
              color={debugMode ? "amber" : "gray"}
              size="2"
              onClick={onToggleDebug}
              aria-label={debugMode ? "Disable debug mode" : "Enable debug mode"}
            >
              <Bug size={16} />
            </IconButton>
            <IconButton
              variant="ghost"
              size="2"
              onClick={() => setSoundEnabled(!soundEnabled)}
              aria-label={soundEnabled ? "Mute sounds" : "Unmute sounds"}
            >
              {soundEnabled ? <SpeakerHigh size={16} /> : <SpeakerSlash size={16} />}
            </IconButton>
          </>
        )}
        <Box style={{ marginLeft: "auto" }}>
          <Tooltip content={collapsed ? "Expand sidebar" : "Collapse sidebar"}>
            <IconButton
              variant="ghost"
              size="1"
              aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
              onClick={() => setCollapsed(!collapsed)}
            >
              {collapsed ? <CaretRight size={14} /> : <CaretLeft size={14} />}
            </IconButton>
          </Tooltip>
        </Box>
      </Flex>

      <NewSessionDialog
        open={newSessionOpen}
        onOpenChange={setNewSessionOpen}
        preselectedProject={currentProject}
      />
    </Box>
  );
}
