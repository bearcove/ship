import { useId, useMemo, useState } from "react";
import { Badge, Box, Code, Flex, Text } from "@radix-ui/themes";
import { CaretDown, CaretRight } from "@phosphor-icons/react";
import type { TaskRecord, TaskStatus, WorktreeDiffStats } from "../generated/ship";

const STATUS_COLOR = {
  Assigned: "blue",
  Working: "blue",
  ReviewPending: "amber",
  SteerPending: "amber",
  Accepted: "green",
  Cancelled: "gray",
} as const;

function summaryTitle(task: TaskRecord | null): string {
  return task?.title || "No active task";
}

function TaskStatusBadge({ status }: { status: TaskStatus }) {
  return (
    <Badge color={STATUS_COLOR[status.tag]} size="1" variant="soft">
      {status.tag}
    </Badge>
  );
}

function TaskListItem({ task, label }: { task: TaskRecord; label: string }) {
  return (
    <Flex
      direction="column"
      gap="1"
      style={{
        padding: "var(--space-2) 0",
        borderTop: "1px solid var(--gray-a4)",
        minWidth: 0,
      }}
    >
      <Flex align="center" gap="2" wrap="wrap">
        <Text size="1" color="gray">
          {label}
        </Text>
        <TaskStatusBadge status={task.status} />
      </Flex>
      <Text size="2" weight="medium" style={{ lineHeight: 1.35 }}>
        {task.title}
      </Text>
      <Text size="1" color="gray" style={{ lineHeight: 1.4 }}>
        {task.description}
      </Text>
    </Flex>
  );
}

interface Props {
  liveTask: TaskRecord | null;
  taskHistory: TaskRecord[];
  branchName: string;
  diffStats: WorktreeDiffStats | null;
  tasksDone: number;
  tasksTotal: number;
}

// r[view.task-panel]
export function SessionTaskDrawer({
  liveTask,
  taskHistory,
  branchName,
  diffStats,
  tasksDone,
  tasksTotal,
}: Props) {
  const [expanded, setExpanded] = useState(false);
  const contentId = useId();
  const history = useMemo(() => [...taskHistory].reverse(), [taskHistory]);
  const summary = summaryTitle(liveTask);

  return (
    <Box
      data-testid="session-task-drawer"
      style={{
        borderBottom: "1px solid var(--gray-a4)",
        background: "linear-gradient(180deg, var(--gray-a2), transparent 180%)",
        flexShrink: 0,
      }}
    >
      <button
        type="button"
        aria-expanded={expanded}
        aria-controls={contentId}
        data-testid="session-task-drawer-toggle"
        onClick={() => setExpanded((value) => !value)}
        style={{
          display: "flex",
          alignItems: "center",
          gap: "var(--space-3)",
          width: "100%",
          padding: "var(--space-2) var(--space-3)",
          border: 0,
          background: "transparent",
          color: "inherit",
          textAlign: "left",
          cursor: "pointer",
        }}
      >
        {expanded ? (
          <CaretDown size={12} style={{ color: "var(--gray-10)", flexShrink: 0 }} />
        ) : (
          <CaretRight size={12} style={{ color: "var(--gray-10)", flexShrink: 0 }} />
        )}
        <Flex direction="column" gap="1" style={{ minWidth: 0, flex: 1 }}>
          <Flex align="center" gap="2" wrap="wrap">
            <Text
              size="2"
              weight="medium"
              data-testid="session-task-drawer-title"
              style={{ minWidth: 0, lineHeight: 1.35 }}
            >
              {summary}
            </Text>
            <Text size="1" color="gray">
              {tasksDone}/{tasksTotal}
            </Text>
          </Flex>
          <Flex
            align="center"
            gap="1"
            wrap="wrap"
            data-testid="session-task-drawer-progress"
            aria-label={`Task progress ${tasksDone} of ${tasksTotal}`}
          >
            {tasksTotal > 0 ? (
              Array.from({ length: tasksTotal }, (_, index) => {
                const complete = index < tasksDone;
                return (
                  <span
                    key={index}
                    data-testid="session-task-drawer-dot"
                    data-complete={complete ? "true" : "false"}
                    style={{
                      width: 8,
                      height: 8,
                      borderRadius: "999px",
                      background: complete ? "var(--accent-9)" : "var(--gray-6)",
                      flexShrink: 0,
                    }}
                  />
                );
              })
            ) : (
              <Text size="1" color="gray">
                No tasks yet
              </Text>
            )}
          </Flex>
        </Flex>
      </button>
      {expanded && (
        <Box
          id={contentId}
          data-testid="session-task-drawer-content"
          style={{
            padding: "0 var(--space-3) var(--space-3)",
            display: "flex",
            flexDirection: "column",
            gap: "var(--space-3)",
          }}
        >
          <Flex align="center" gap="2" wrap="wrap">
            <Text size="1" color="gray">
              Branch
            </Text>
            <Code variant="ghost" size="1">
              {branchName}
            </Code>
            {diffStats && (
              <>
                <Text size="1" color="gray">
                  ·
                </Text>
                <Text
                  size="1"
                  style={{ color: "var(--green-10)", fontFamily: "var(--code-font-family)" }}
                >
                  +{String(diffStats.lines_added)}
                </Text>
                <Text
                  size="1"
                  style={{ color: "var(--red-10)", fontFamily: "var(--code-font-family)" }}
                >
                  -{String(diffStats.lines_removed)}
                </Text>
                {diffStats.files_changed > 0n && (
                  <Text size="1" color="gray">
                    · {String(diffStats.files_changed)} files
                  </Text>
                )}
              </>
            )}
          </Flex>

          <Flex direction="column" gap="1">
            <Text size="1" weight="bold" color="gray">
              Active
            </Text>
            {liveTask ? (
              <TaskListItem task={liveTask} label="Current task" />
            ) : (
              <Text size="2" color="gray" style={{ paddingTop: "var(--space-2)" }}>
                No active task
              </Text>
            )}
          </Flex>

          <Flex direction="column" gap="1">
            <Text size="1" weight="bold" color="gray">
              History
            </Text>
            {history.length > 0 ? (
              history.map((task) => (
                <TaskListItem key={task.id} task={task} label="Previous task" />
              ))
            ) : (
              <Text size="2" color="gray" style={{ paddingTop: "var(--space-2)" }}>
                No completed tasks yet
              </Text>
            )}
          </Flex>
        </Box>
      )}
    </Box>
  );
}
