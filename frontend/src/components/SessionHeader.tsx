import { useId, useMemo, useState } from "react";
import { Badge, Box, Code, DropdownMenu, Flex, IconButton, Popover, Spinner, Text } from "@radix-ui/themes";
import {
  Archive,
  CaretDown,
  CaretRight,
  ChatsCircle,
  CheckCircle,
  Circle,
  DotsThree,
  Plus,
  XCircle,
} from "@phosphor-icons/react";
import { useNavigate } from "react-router-dom";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type {
  AgentSnapshot,
  PlanStep,
  PlanStepStatus,
  SessionSummary,
  TaskRecord,
  TaskStatus,
  WorktreeDiffStats,
} from "../generated/ship";
import { AgentModelPicker } from "./AgentModelPicker";
import { AgentEffortPicker } from "./AgentEffortPicker";
import { AgentKindIcon } from "./AgentKindIcon";
import { MarkdownCodeBlock } from "./blocks/TextBlock";
import {
  feedBubble,
  planStepRow,
  planStepText,
  sessionHeaderAgentsRow,
  sessionHeaderCaret,
  sessionHeaderHistoryCaret,
  sessionHeaderDiffAdd,
  sessionHeaderDiffFlex,
  sessionHeaderDiffRemove,
  sessionHeaderDot,
  sessionHeaderExpanded,
  sessionHeaderHistoryBody,
  sessionHeaderHistoryBtn,
  sessionHeaderHistoryItem,
  sessionHeaderHistoryTitle,
  sessionHeaderHistoryTitleRow,
  sessionHeaderInlineAvatar,
  sessionHeaderPanelInner,
  sessionHeaderProgressFlex,
  sessionHeaderRoot,
  sessionHeaderRow1,
  sessionHeaderRow2,
  sessionHeaderRow2Title,
  sessionHeaderSectionLabel,
  sessionHeaderStepIconWrap,
  sessionHeaderStepText,
  sessionHeaderTitle,
  sessionSwitcherList,
  sessionSwitcherRow,
  sessionSwitcherRowSub,
  sessionSwitcherRowTitle,
  taskDescriptionRoot,
} from "../styles/session-view.css";
import { NewSessionDialog } from "../pages/SessionListPage";
import { useSessionList } from "../hooks/useSessionList";

// ─── helpers ──────────────────────────────────────────────────────────────────

function StepIcon({ status }: { status: PlanStepStatus }) {
  switch (status.tag) {
    case "Pending":
      return <Circle size={12} color="var(--gray-8)" />;
    case "InProgress":
      return <Spinner size="1" />;
    case "Completed":
      return <CheckCircle size={12} weight="fill" color="var(--green-9)" />;
    case "Failed":
      return <XCircle size={12} weight="fill" color="var(--red-9)" />;
  }
}

const STATUS_COLOR = {
  Assigned: "blue",
  Working: "blue",
  ReviewPending: "amber",
  SteerPending: "amber",
  Accepted: "green",
  Cancelled: "gray",
} as const;

function statusLabel(status: TaskStatus | null): string {
  if (!status) return "Idle";
  switch (status.tag) {
    case "ReviewPending": return "Review";
    case "SteerPending": return "Steer";
    case "Working": return "Working";
    case "Assigned": return "Starting";
    case "Accepted": return "Done";
    case "Cancelled": return "Cancelled";
  }
}

function sortSessions(sessions: SessionSummary[]): SessionSummary[] {
  const priority = (s: SessionSummary) => {
    const tag = s.task_status?.tag;
    if (tag === "ReviewPending" || tag === "SteerPending") return 0;
    if (tag === "Working" || tag === "Assigned") return 1;
    return 2;
  };
  return [...sessions].sort((a, b) => priority(a) - priority(b));
}

function TaskStatusBadge({ status }: { status: TaskStatus }) {
  return (
    <Badge color={STATUS_COLOR[status.tag]} size="1" variant="soft">
      {status.tag}
    </Badge>
  );
}

const titleMdComponents: React.ComponentProps<typeof ReactMarkdown>["components"] = {
  p: ({ children }) => <>{children}</>,
};

const mdComponents: React.ComponentProps<typeof ReactMarkdown>["components"] = {
  code({ children, className }: { children?: React.ReactNode; className?: string }) {
    const raw = String(children ?? "");
    const isBlock =
      Boolean(className?.startsWith("language-")) || raw.includes("\n") || raw.endsWith("\n");
    if (isBlock) return <MarkdownCodeBlock className={className} code={raw.replace(/\n$/, "")} />;
    return <Code size="1">{children}</Code>;
  },
};

function HistoryItem({ task }: { task: TaskRecord }) {
  const [expanded, setExpanded] = useState(false);
  const headerId = useId();
  const bodyId = useId();

  return (
    <div className={sessionHeaderHistoryItem}>
      <button
        type="button"
        id={headerId}
        aria-expanded={expanded}
        aria-controls={bodyId}
        className={sessionHeaderHistoryBtn}
        onClick={() => setExpanded((v) => !v)}
      >
        {expanded ? (
          <CaretDown size={11} className={sessionHeaderHistoryCaret} />
        ) : (
          <CaretRight size={11} className={sessionHeaderHistoryCaret} />
        )}
        <div className={sessionHeaderHistoryTitleRow}>
          <Text size="1" weight="medium" className={sessionHeaderHistoryTitle}>
            <ReactMarkdown remarkPlugins={[remarkGfm]} components={titleMdComponents}>
              {task.title}
            </ReactMarkdown>
          </Text>
          <TaskStatusBadge status={task.status} />
        </div>
      </button>
      {expanded && (
        <div
          id={bodyId}
          role="region"
          aria-labelledby={headerId}
          className={sessionHeaderHistoryBody}
        >
          <div className={`${feedBubble} ${taskDescriptionRoot}`}>
            <ReactMarkdown remarkPlugins={[remarkGfm]} components={mdComponents}>
              {task.description}
            </ReactMarkdown>
          </div>
        </div>
      )}
    </div>
  );
}

// ─── main component ───────────────────────────────────────────────────────────

interface Props {
  sessionId: string;
  project: string;
  title: string | null;
  branchName: string;
  captain: AgentSnapshot | null;
  mate: AgentSnapshot | null;
  liveTask: TaskRecord | null;
  taskHistory: TaskRecord[];
  planSteps: PlanStep[];
  matePlan: PlanStep[] | null;
  diffStats: WorktreeDiffStats | null;
  canArchiveSession: boolean;
  onArchive: () => void;
  archiving: boolean;
}

export function SessionHeader({
  sessionId,
  project,
  title,
  branchName,
  captain,
  mate,
  liveTask,
  taskHistory,
  planSteps,
  matePlan,
  diffStats,
  canArchiveSession,
  onArchive,
  archiving,
}: Props) {
  const hasActivePlan = !!matePlan && matePlan.length > 0;
  const [expanded, setExpanded] = useState(hasActivePlan);
  const inProgressStep =
    matePlan?.find((s) => s.status.tag === "InProgress") ??
    matePlan?.find((s) => s.status.tag === "Pending") ??
    null;
  const [newSessionOpen, setNewSessionOpen] = useState(false);
  const [switcherOpen, setSwitcherOpen] = useState(false);
  const contentId = useId();
  const navigate = useNavigate();
  const allSessions = useSessionList();

  const displayTitle = liveTask?.title ?? title ?? branchName;
  const history = useMemo(() => [...taskHistory].reverse(), [taskHistory]);

  const progressDots =
    planSteps.length > 0 ? (
      <div
        className={sessionHeaderProgressFlex}
        aria-label={`${planSteps.filter((s) => s.status.tag === "Completed").length} of ${planSteps.length} steps done`}
      >
        {planSteps.map((step, i) => (
          <span
            key={i}
            className={sessionHeaderDot}
            data-complete={step.status.tag === "Completed" ? "true" : "false"}
          />
        ))}
      </div>
    ) : null;

  const diffBadge = diffStats ? (
    <div className={sessionHeaderDiffFlex}>
      <Text size="1" className={sessionHeaderDiffAdd}>
        +{String(diffStats.lines_added)}
      </Text>
      <Text size="1" className={sessionHeaderDiffRemove}>
        -{String(diffStats.lines_removed)}
      </Text>
    </div>
  ) : null;

  return (
    <>
      <div className={sessionHeaderRoot}>
        {/* Row 1: title + menu */}
        <div className={sessionHeaderRow1} onClick={() => setExpanded((v) => !v)}>
          <Text size="3" weight="medium" className={sessionHeaderTitle}>
            {displayTitle}
          </Text>
          <Popover.Root open={switcherOpen} onOpenChange={setSwitcherOpen}>
            <Popover.Trigger asChild>
              <IconButton
                variant="ghost"
                color="gray"
                size="2"
                aria-label="Switch session"
                onClick={(e) => e.stopPropagation()}
              >
                <ChatsCircle size={18} />
              </IconButton>
            </Popover.Trigger>
            <Popover.Content align="end" size="1" style={{ padding: "var(--space-1)" }}>
              <div className={sessionSwitcherList}>
                {sortSessions(allSessions).map((session) => {
                  const isActive = session.id === sessionId;
                  const isActiveTask = ["Working", "Assigned", "ReviewPending", "SteerPending"].includes(
                    session.task_status?.tag ?? "",
                  );
                  const rowTitle =
                    isActiveTask && session.current_task_title
                      ? session.current_task_title
                      : (session.title ?? session.branch_name);
                  return (
                    <div
                      key={session.id}
                      className={sessionSwitcherRow}
                      data-active={isActive ? "true" : "false"}
                      onClick={() => {
                        navigate(`/sessions/${session.slug}`);
                        setSwitcherOpen(false);
                      }}
                    >
                      <div className={sessionSwitcherRowTitle}>{rowTitle}</div>
                      <div className={sessionSwitcherRowSub}>
                        {session.project} · {statusLabel(session.task_status)}
                        {session.tasks_total > 0 && (
                          <> · {session.tasks_done}/{session.tasks_total}</>
                        )}
                        {session.diff_stats &&
                          (session.diff_stats.lines_added > 0 || session.diff_stats.lines_removed > 0) && (
                            <>
                              {" · "}
                              <span style={{ color: "var(--green-10)" }}>+{String(session.diff_stats.lines_added)}</span>
                              {" "}
                              <span style={{ color: "var(--red-10)" }}>-{String(session.diff_stats.lines_removed)}</span>
                            </>
                          )}
                      </div>
                    </div>
                  );
                })}
              </div>
            </Popover.Content>
          </Popover.Root>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger asChild>
              <IconButton
                variant="ghost"
                color="gray"
                size="2"
                aria-label="Session menu"
                onClick={(e) => e.stopPropagation()}
              >
                <DotsThree size={18} weight="bold" />
              </IconButton>
            </DropdownMenu.Trigger>
            <DropdownMenu.Content size="2" align="end">
              <DropdownMenu.Item onClick={() => setNewSessionOpen(true)}>
                <Plus size={13} />
                New session
              </DropdownMenu.Item>
              {canArchiveSession && (
                <>
                  <DropdownMenu.Separator />
                  <DropdownMenu.Item color="red" onClick={onArchive} disabled={archiving}>
                    <Archive size={13} />
                    {archiving ? "Archiving…" : "Archive session"}
                  </DropdownMenu.Item>
                </>
              )}
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        </div>

        {/* Row 2: in-progress step + progress + diff badge + chevron */}
        <div className={sessionHeaderRow2} onClick={() => setExpanded((v) => !v)}>
          <Flex align="center" gap="1" className={sessionHeaderRow2Title} style={{ minWidth: 0 }}>
            {inProgressStep?.status.tag === "InProgress" && <Spinner size="1" flexShrink="0" />}
            <Text size="1" color="gray" style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {inProgressStep?.title || inProgressStep?.description || "No plan yet"}
            </Text>
          </Flex>
          <Flex align="center" gap="2">
            {progressDots}
            {diffBadge}
            {expanded ? (
              <CaretDown size={11} className={sessionHeaderCaret} />
            ) : (
              <CaretRight size={11} className={sessionHeaderCaret} />
            )}
          </Flex>
        </div>

        {/* Expanded panel */}
        <div id={contentId} className={sessionHeaderExpanded} data-open={expanded}>
          <div className={sessionHeaderPanelInner}>
            {/* Plan */}
            <div>
              <Text className={sessionHeaderSectionLabel} as="div">
                Plan
              </Text>
              {matePlan && matePlan.length > 0 ? (
                <Flex direction="column" gap="1">
                  {matePlan.map((step, i) => (
                    <Flex key={i} align="start" gap="2" className={planStepRow}>
                      <div className={sessionHeaderStepIconWrap}>
                        <StepIcon status={step.status} />
                      </div>
                      <Text
                        size="2"
                        as="span"
                        className={`${planStepText} ${sessionHeaderStepText}`}
                        data-status={step.status.tag}
                      >
                        {step.title || step.description}
                      </Text>
                    </Flex>
                  ))}
                </Flex>
              ) : (
                <Text size="2" color="gray">
                  No plan yet.
                </Text>
              )}
            </div>

            {/* Agents + Branch */}
            <Flex align="center" gap="2" className={sessionHeaderAgentsRow}>
              {captain && (
                <>
                  <Box className={sessionHeaderInlineAvatar}>
                    <AgentKindIcon kind={captain.kind} />
                  </Box>
                  <AgentModelPicker sessionId={sessionId} agent={captain} />
                  <AgentEffortPicker sessionId={sessionId} agent={captain} />
                </>
              )}
              {mate && (
                <>
                  <Box className={sessionHeaderInlineAvatar}>
                    <AgentKindIcon kind={mate.kind} />
                  </Box>
                  <AgentModelPicker sessionId={sessionId} agent={mate} />
                  <AgentEffortPicker sessionId={sessionId} agent={mate} />
                </>
              )}
              <Box style={{ flex: 1 }} />
              <Code variant="ghost" size="1">
                {branchName}
              </Code>
              {diffStats && (
                <>
                  <Text size="1" className={sessionHeaderDiffAdd}>
                    +{String(diffStats.lines_added)}
                  </Text>
                  <Text size="1" className={sessionHeaderDiffRemove}>
                    -{String(diffStats.lines_removed)}
                  </Text>
                </>
              )}
            </Flex>

            {/* Current task description */}
            {liveTask && (
              <div>
                <Text className={sessionHeaderSectionLabel} as="div">
                  Current task
                </Text>
                <div className={`${feedBubble} ${taskDescriptionRoot}`}>
                  <ReactMarkdown remarkPlugins={[remarkGfm]} components={mdComponents}>
                    {liveTask.description}
                  </ReactMarkdown>
                </div>
              </div>
            )}

            {/* Task history */}
            {history.length > 0 && (
              <div>
                <Text className={sessionHeaderSectionLabel} as="div">
                  History
                </Text>
                <Flex direction="column">
                  {history.map((task) => (
                    <HistoryItem key={task.id} task={task} />
                  ))}
                </Flex>
              </div>
            )}
          </div>
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
