import { useEffect, useId, useMemo, useState } from "react";
import { Badge, Box, Code, Flex, Popover, Spinner, Text } from "@radix-ui/themes";
import {
  Archive,
  CaretDown,
  CaretRight,
  ChatsCircle,
  CheckCircle,
  Circle,
  Plus,
  XCircle,
  Terminal,
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
import { fixMarkdownBackticks } from "../utils/fixMarkdownBackticks";
import {
  feedBubble,
  planStepRow,
  planStepText,
  sessionHeaderAgentsRow,
  sessionHeaderCaret,
  sessionHeaderCollapsedArea,
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
  sessionHeaderRows,
  sessionHeaderSectionLabel,
  sessionHeaderSideButton,
  sessionHeaderSideButtonDesktopOnly,
  sessionHeaderSideButtons,
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
import { getShipClient } from "../api/client";
import { sortSessions } from "../pages/session-list-utils";

const sideButtonSize = 24;

// ─── helpers ──────────────────────────────────────────────────────────────────

function formatElapsed(isoStart: string): string {
  const secs = Math.floor((Date.now() - new Date(isoStart).getTime()) / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ${secs % 60}s`;
  const hrs = Math.floor(mins / 60);
  return `${hrs}h ${mins % 60}m`;
}

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
              {fixMarkdownBackticks(task.title)}
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
              {fixMarkdownBackticks(task.description)}
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
  onArchive,
  archiving,
}: Props) {
  const hasActivePlan = !!matePlan && matePlan.length > 0;
  const [expanded, setExpanded] = useState(false);
  const activePlan = hasActivePlan ? matePlan! : planSteps;
  const inProgressStep =
    activePlan.find((s) => s.status.tag === "InProgress") ??
    activePlan.find((s) => s.status.tag === "Pending") ??
    null;
  const [newSessionOpen, setNewSessionOpen] = useState(false);
  const [switcherOpen, setSwitcherOpen] = useState(false);
  const contentId = useId();
  useEffect(() => { setExpanded(false); }, [sessionId]);
  const navigate = useNavigate();
  const allSessions = useSessionList();

  const elapsedSource = inProgressStep?.started_at ?? liveTask?.assigned_at ?? null;
  const [elapsedLabel, setElapsedLabel] = useState<string | null>(
    elapsedSource ? formatElapsed(elapsedSource) : null,
  );
  useEffect(() => {
    if (!elapsedSource) {
      setElapsedLabel(null);
      return;
    }
    setElapsedLabel(formatElapsed(elapsedSource));
    const id = setInterval(() => setElapsedLabel(formatElapsed(elapsedSource)), 1000);
    return () => clearInterval(id);
  }, [elapsedSource]);

  const hasDisplayTitle = !!(liveTask?.title ?? title);
  const displayTitle = hasDisplayTitle ? (liveTask?.title ?? title!) : "Untitled";
  const history = useMemo(() => [...taskHistory].reverse(), [taskHistory]);

  const progressDots =
    activePlan.length > 0 ? (
      <div
        className={sessionHeaderProgressFlex}
        aria-label={`${activePlan.filter((s) => s.status.tag === "Completed").length} of ${activePlan.length} steps done`}
      >
        {activePlan.map((step, i) => (
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
      <div className={sessionHeaderRoot} style={expanded ? { height: "100vh" } : undefined}>
        {/* Collapsed header: rows + side buttons */}
        <div className={sessionHeaderCollapsedArea} onClick={() => setExpanded((v) => !v)}>
          <div className={sessionHeaderRows}>
            {/* Row 1: title + caret */}
            <div className={sessionHeaderRow1}>
              <Text size="3" weight="medium" className={sessionHeaderTitle} color={hasDisplayTitle ? undefined : "gray"}>
                {displayTitle}
              </Text>
              {expanded ? (
                <CaretDown size={11} className={sessionHeaderCaret} />
              ) : (
                <CaretRight size={11} className={sessionHeaderCaret} />
              )}
            </div>

            {/* Row 2: in-progress step + progress + diff badge */}
            <div className={sessionHeaderRow2}>
              <Flex align="center" gap="1" className={sessionHeaderRow2Title} style={{ minWidth: 0 }}>
                {inProgressStep?.status.tag === "InProgress" && <Spinner size="1" style={{ flexShrink: 0 }} />}
                <Text size="1" color="gray" style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {inProgressStep?.title || inProgressStep?.description || "No plan yet"}
                </Text>
              </Flex>
              <Flex align="center" gap="2">
                {elapsedLabel && (
                  <Text size="1" color="gray" style={{ flexShrink: 0 }}>
                    {elapsedLabel}
                  </Text>
                )}
                {progressDots}
                {diffBadge}
              </Flex>
            </div>
          </div>{/* end sessionHeaderRows */}

          {/* Side buttons: switcher + menu (always) + Zed/iTerm (desktop only) */}
          <Flex align="stretch" style={{ borderLeft: "1px solid var(--gray-a4)" }}>
            <button
              className={`${sessionHeaderSideButton} ${sessionHeaderSideButtonDesktopOnly}`}
              title="Open in Zed"
              aria-label="Open in Zed"
              onClick={(e) => {
                e.stopPropagation();
                void getShipClient().then((c) => c.openInEditor(sessionId));
              }}
            >
              <svg width={sideButtonSize} height={sideButtonSize} viewBox="-12 -12 114 114" fill="none" xmlns="http://www.w3.org/2000/svg">
                <path fillRule="evenodd" clipRule="evenodd" d="M8.4375 5.625C6.8842 5.625 5.625 6.8842 5.625 8.4375V70.3125H0V8.4375C0 3.7776 3.7776 0 8.4375 0H83.7925C87.551 0 89.4333 4.5442 86.7756 7.20186L40.3642 53.6133H53.4375V47.8125H59.0625V55.0195C59.0625 57.3495 57.1737 59.2383 54.8438 59.2383H34.7392L25.0712 68.9062H68.9062V33.75H74.5312V68.9062C74.5312 72.0128 72.0128 74.5312 68.9062 74.5312H19.4462L9.60248 84.375H81.5625C83.1158 84.375 84.375 83.1158 84.375 81.5625V19.6875H90V81.5625C90 86.2224 86.2224 90 81.5625 90H6.20749C2.44898 90 0.566723 85.4558 3.22438 82.7981L49.46 36.5625H36.5625V42.1875H30.9375V35.1562C30.9375 32.8263 32.8263 30.9375 35.1562 30.9375H55.085L64.9288 21.0938H21.0938V56.25H15.4688V21.0938C15.4688 17.9871 17.9871 15.4688 21.0938 15.4688H70.5538L80.3975 5.625H8.4375Z" fill="currentColor" />
              </svg>
            </button>
            <button
              className={`${sessionHeaderSideButton} ${sessionHeaderSideButtonDesktopOnly}`}
              title="Open in iTerm"
              aria-label="Open in iTerm"
              onClick={(e) => {
                e.stopPropagation();
                void getShipClient().then((c) => c.openInTerminal(sessionId));
              }}
            >
              <Terminal size={sideButtonSize} />
            </button>
            <Popover.Root open={switcherOpen} onOpenChange={setSwitcherOpen}>
              <Popover.Trigger asChild>
                <button
                  className={sessionHeaderSideButton}
                  aria-label="Switch session"
                  onClick={(e) => e.stopPropagation()}
                >
                  <ChatsCircle size={sideButtonSize} />
                </button>
              </Popover.Trigger>
              <Popover.Content align="end" size="1" style={{ padding: "var(--space-1)" }}>
                <Flex
                  align="center"
                  gap="2"
                  px="2"
                  py="1"
                  style={{ cursor: "pointer", borderRadius: "var(--radius-1)" }}
                  className="rt-reset"
                  onClick={() => {
                    setNewSessionOpen(true);
                    setSwitcherOpen(false);
                  }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = "var(--gray-a3)")}
                  onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
                >
                  <Plus size={14} color="var(--gray-11)" />
                  <Text size="2" color="gray">New session</Text>
                </Flex>
                <div style={{ height: 1, background: "var(--gray-a4)", margin: "var(--space-1) 0" }} />
                <div className={sessionSwitcherList}>
                  {sortSessions(allSessions).map((session) => {
                    const isActive = session.id === sessionId;
                    const isActiveTask = ["Working", "Assigned", "ReviewPending", "SteerPending"].includes(
                      session.task_status?.tag ?? "",
                    );
                    const hasRowTitle = isActiveTask ? !!session.current_task_title : !!session.title;
                    const rowTitle = hasRowTitle
                      ? (isActiveTask && session.current_task_title
                        ? session.current_task_title
                        : session.title!)
                      : "Untitled";
                    const mateState = session.mate.state;
                    const currentStep =
                      mateState.tag === "Working" && mateState.plan
                        ? (mateState.plan.find((s) => s.status.tag === "InProgress") ??
                          mateState.plan.find((s) => s.status.tag === "Pending") ??
                          null)
                        : null;
                    const stepLabel = currentStep?.title || currentStep?.description || null;
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
                        <div className={sessionSwitcherRowTitle} style={hasRowTitle ? undefined : { color: "var(--gray-9)" }}>{rowTitle}</div>
                        <div className={sessionSwitcherRowSub}>
                          {session.project} · {statusLabel(session.task_status)}
                          {stepLabel && <> · {stepLabel}</>}
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
                <div style={{ height: 1, background: "var(--gray-a4)", margin: "var(--space-1) 0" }} />
                <Flex
                  align="center"
                  gap="2"
                  px="2"
                  py="1"
                  style={{
                    cursor: archiving ? "default" : "pointer",
                    borderRadius: "var(--radius-1)",
                    opacity: archiving ? 0.5 : 1,
                  }}
                  onClick={() => { if (!archiving) onArchive(); }}
                  onMouseEnter={(e) => { if (!archiving) e.currentTarget.style.background = "var(--gray-a3)"; }}
                  onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
                >
                  <Archive size={14} color="var(--red-9)" />
                  <Text size="2" color="red">{archiving ? "Archiving…" : "Archive session"}</Text>
                </Flex>
              </Popover.Content>
            </Popover.Root>

          </Flex>
        </div>{/* end sessionHeaderCollapsedArea */}

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
              {diffStats && (diffStats.uncommitted_lines_added > 0n || diffStats.uncommitted_lines_removed > 0n) && (
                <>
                  <Text size="1" color="amber">+{String(diffStats.uncommitted_lines_added)}</Text>
                  <Text size="1" color="amber">-{String(diffStats.uncommitted_lines_removed)}</Text>
                </>
              )}
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
                    {fixMarkdownBackticks(liveTask.description)}
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
      </div >

      <NewSessionDialog
        open={newSessionOpen}
        onOpenChange={setNewSessionOpen}
        preselectedProject={project}
      />
    </>
  );
}
