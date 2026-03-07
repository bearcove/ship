import { useState } from "react";
import { Link } from "react-router-dom";
import { Box, IconButton } from "@radix-ui/themes";
import { Plus } from "@phosphor-icons/react";
import type { SessionSummary, TaskStatus } from "../generated/ship";
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
}

// r[ui.session-list.nav]
export function SessionSidebar({ sessions, currentSessionId }: Props) {
  const [newSessionOpen, setNewSessionOpen] = useState(false);

  return (
    <Box className={sidebarRoot}>
      <Box className={sidebarScrollArea}>
        {sessions.map((session) => {
          const isActive = session.id === currentSessionId;
          const rawDesc = session.current_task_description;
          const desc = rawDesc
            ? rawDesc.length > 50
              ? `${rawDesc.slice(0, 47)}…`
              : rawDesc
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

      <Box className={sidebarFooter}>
        <IconButton
          variant="ghost"
          size="2"
          aria-label="New session"
          onClick={() => setNewSessionOpen(true)}
        >
          <Plus size={16} />
        </IconButton>
      </Box>

      <NewSessionDialog open={newSessionOpen} onOpenChange={setNewSessionOpen} />
    </Box>
  );
}
