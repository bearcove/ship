import { useId, useMemo, useState } from "react";
import { Box, Badge, Code, DropdownMenu, Flex, IconButton, Spinner, Text } from "@radix-ui/themes";
import {
  Archive,
  CaretDown,
  CaretRight,
  CheckCircle,
  Circle,
  DotsThree,
  Plus,
  XCircle,
} from "@phosphor-icons/react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type {
  AgentSnapshot,
  PlanStep,
  PlanStepStatus,
  TaskRecord,
  TaskStatus,
  WorktreeDiffStats,
} from "../generated/ship";
import { AgentModelPicker } from "./AgentModelPicker";
import { AgentEffortPicker } from "./AgentEffortPicker";
import { MarkdownCodeBlock } from "./blocks/TextBlock";
import {
  planStepRow,
  planStepText,
  sessionHeaderAgentLabel,
  sessionHeaderAgentRow,
  sessionHeaderBranchMeta,
  sessionHeaderExpanded,
  sessionHeaderPanelInner,
  sessionHeaderRoot,
  sessionHeaderRow1,
  sessionHeaderRow2,
  sessionHeaderRow2Title,
  sessionHeaderSectionLabel,
  sessionHeaderTitle,
  taskDescriptionRoot,
} from "../styles/session-view.css";
import { NewSessionDialog } from "../pages/SessionListPage";

// ─── small helpers ────────────────────────────────────────────────────────────

function StepIcon({ status }: { status: PlanStepStatus }) {
  switch (status.tag) {
    case "Pending":
      return <Circle size={12} style={{ color: "var(--gray-8)", flexShrink: 0 }} />;
    case "InProgress":
      return <Spinner size="1" />;
    case "Completed":
      return <CheckCircle size={12} weight="fill" style={{ color: "var(--green-9)", flexShrink: 0 }} />;
    case "Failed":
      return <XCircle size={12} weight="fill" style={{ color: "var(--red-9)", flexShrink: 0 }} />;
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

function TaskStatusBadge({ status }: { status: TaskStatus }) {
  return (
    <Badge color={STATUS_COLOR[status.tag]} size="1" variant="soft" style={{ flexShrink: 0 }}>
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
    <Flex direction="column" style={{ borderTop: "1px solid var(--gray-a4)", minWidth: 0 }}>
      <button
        type="button"
        id={headerId}
        aria-expanded={expanded}
        aria-controls={bodyId}
        onClick={() => setExpanded((v) => !v)}
        style={{
          display: "flex",
          alignItems: "flex-start",
          gap: "var(--space-2)",
          width: "100%",
          padding: "var(--space-2) 0",
          border: 0,
          background: "transparent",
          color: "inherit",
          textAlign: "left",
          cursor: "pointer",
        }}
      >
        {expanded ? (
          <CaretDown size={11} style={{ color: "var(--gray-10)", flexShrink: 0, marginTop: 3 }} />
        ) : (
          <CaretRight size={11} style={{ color: "var(--gray-10)", flexShrink: 0, marginTop: 3 }} />
        )}
        <Flex align="center" gap="2" style={{ flex: 1, minWidth: 0, flexWrap: "wrap" }}>
          <Text size="1" weight="medium" style={{ flex: 1, minWidth: 0, lineHeight: 1.35 }}>
            <ReactMarkdown remarkPlugins={[remarkGfm]} components={titleMdComponents}>
              {task.title}
            </ReactMarkdown>
          </Text>
          <TaskStatusBadge status={task.status} />
        </Flex>
      </button>
      {expanded && (
        <Box
          id={bodyId}
          role="region"
          aria-labelledby={headerId}
          style={{ paddingLeft: "calc(11px + var(--space-2))", paddingBottom: "var(--space-2)" }}
        >
          <Box
            style={{
              background: "var(--gray-a2)",
              border: "1px solid var(--gray-a4)",
              borderRadius: "var(--radius-3)",
              padding: "var(--space-3)",
            }}
          >
            <div className={taskDescriptionRoot}>
              <ReactMarkdown remarkPlugins={[remarkGfm]} components={mdComponents}>
                {task.description}
              </ReactMarkdown>
            </div>
          </Box>
        </Box>
      )}
    </Flex>
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
  const [expanded, setExpanded] = useState(hasActivePlan);
  const [newSessionOpen, setNewSessionOpen] = useState(false);
  const contentId = useId();

  const displayTitle = title ?? branchName;
  const history = useMemo(() => [...taskHistory].reverse(), [taskHistory]);

  const progressDots =
    planSteps.length > 0 ? (
      <Flex
        align="center"
        gap="1"
        aria-label={`${planSteps.filter((s) => s.status.tag === "Completed").length} of ${planSteps.length} steps done`}
        style={{ flexShrink: 0 }}
      >
        {planSteps.map((step, i) => (
          <span
            key={i}
            style={{
              width: 7,
              height: 7,
              borderRadius: "999px",
              background:
                step.status.tag === "Completed" ? "var(--accent-9)" : "var(--gray-6)",
              flexShrink: 0,
            }}
          />
        ))}
      </Flex>
    ) : null;

  const diffBadge = diffStats ? (
    <Flex align="center" gap="1" style={{ flexShrink: 0 }}>
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
    </Flex>
  ) : null;

  return (
    <>
      <div className={sessionHeaderRoot}>
        {/* Row 1: title + ⋯ menu */}
        <div className={sessionHeaderRow1}>
          <Text size="2" weight="medium" className={sessionHeaderTitle}>
            {displayTitle}
          </Text>

          <DropdownMenu.Root>
            <DropdownMenu.Trigger>
              <IconButton variant="ghost" color="gray" size="2" aria-label="Session menu">
                <DotsThree size={18} weight="bold" />
              </IconButton>
            </DropdownMenu.Trigger>
            <DropdownMenu.Content size="1" align="end">
              <DropdownMenu.Item onClick={() => setNewSessionOpen(true)}>
                <Plus size={13} />
                New session
              </DropdownMenu.Item>
              <DropdownMenu.Separator />
              <DropdownMenu.Item color="red" onClick={onArchive} disabled={archiving}>
                <Archive size={13} />
                {archiving ? "Archiving…" : "Archive session"}
              </DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        </div>

        {/* Row 2: expand toggle — always visible */}
        <button
          type="button"
          aria-expanded={expanded}
          aria-controls={contentId}
          className={sessionHeaderRow2}
          onClick={() => setExpanded((v) => !v)}
        >
          {expanded ? (
            <CaretDown size={11} style={{ color: "var(--gray-10)", flexShrink: 0 }} />
          ) : (
            <CaretRight size={11} style={{ color: "var(--gray-10)", flexShrink: 0 }} />
          )}

          {liveTask ? (
            <Text size="2" className={sessionHeaderRow2Title}>
              {liveTask.title}
            </Text>
          ) : (
            <Text size="2" color="gray" className={sessionHeaderRow2Title}>
              No active task
            </Text>
          )}

          {progressDots}
          {diffBadge}
        </button>

        {/* Expanded panel */}
        <div
          id={contentId}
          className={sessionHeaderExpanded}
          data-open={expanded}
        >
          <div className={sessionHeaderPanelInner}>

            {/* Plan */}
            {matePlan && matePlan.length > 0 && (
              <div>
                <Text className={sessionHeaderSectionLabel} as="div">Plan</Text>
                <Flex direction="column" gap="1">
                  {matePlan.map((step, i) => (
                    <Flex key={i} align="start" gap="2" className={planStepRow}>
                      <Box style={{ paddingTop: 2, display: "flex" }}>
                        <StepIcon status={step.status} />
                      </Box>
                      <Text
                        size="2"
                        className={planStepText}
                        style={{
                          color:
                            step.status.tag === "Completed"
                              ? "var(--gray-9)"
                              : step.status.tag === "Failed"
                                ? "var(--red-11)"
                                : "var(--gray-12)",
                          textDecoration:
                            step.status.tag === "Completed" ? "line-through" : undefined,
                        }}
                      >
                        {step.title || step.description}
                      </Text>
                    </Flex>
                  ))}
                </Flex>
              </div>
            )}
            {(!matePlan || matePlan.length === 0) && (
              <div>
                <Text className={sessionHeaderSectionLabel} as="div">Plan</Text>
                <Text size="2" color="gray">No plan yet.</Text>
              </div>
            )}

            {/* Agents */}
            {(captain ?? mate) && (
              <div>
                <Text className={sessionHeaderSectionLabel} as="div">Agents</Text>
                <Flex direction="column" gap="2">
                  {captain && (
                    <div className={sessionHeaderAgentRow}>
                      <Text size="1" color="gray" className={sessionHeaderAgentLabel}>Captain</Text>
                      <AgentModelPicker sessionId={sessionId} agent={captain} />
                      <AgentEffortPicker sessionId={sessionId} agent={captain} />
                    </div>
                  )}
                  {mate && (
                    <div className={sessionHeaderAgentRow}>
                      <Text size="1" color="gray" className={sessionHeaderAgentLabel}>Mate</Text>
                      <AgentModelPicker sessionId={sessionId} agent={mate} />
                      <AgentEffortPicker sessionId={sessionId} agent={mate} />
                    </div>
                  )}
                </Flex>
              </div>
            )}

            {/* Branch + diff */}
            <div>
              <Text className={sessionHeaderSectionLabel} as="div">Branch</Text>
              <div className={sessionHeaderBranchMeta}>
                <Code variant="ghost" size="1">{branchName}</Code>
                {diffStats && (
                  <>
                    <Text size="1" color="gray">·</Text>
                    <Text size="1" style={{ color: "var(--green-10)", fontFamily: "var(--code-font-family)" }}>
                      +{String(diffStats.lines_added)}
                    </Text>
                    <Text size="1" style={{ color: "var(--red-10)", fontFamily: "var(--code-font-family)" }}>
                      -{String(diffStats.lines_removed)}
                    </Text>
                    {diffStats.files_changed > 0n && (
                      <Text size="1" color="gray">· {String(diffStats.files_changed)} files</Text>
                    )}
                  </>
                )}
              </div>
            </div>

            {/* Current task description */}
            {liveTask && (
              <div>
                <Text className={sessionHeaderSectionLabel} as="div">Current task</Text>
                <Box
                  style={{
                    background: "var(--gray-a2)",
                    border: "1px solid var(--gray-a4)",
                    borderRadius: "var(--radius-3)",
                    padding: "var(--space-3)",
                  }}
                >
                  <div className={taskDescriptionRoot}>
                    <ReactMarkdown remarkPlugins={[remarkGfm]} components={mdComponents}>
                      {liveTask.description}
                    </ReactMarkdown>
                  </div>
                </Box>
              </div>
            )}

            {/* Task history */}
            {history.length > 0 && (
              <div>
                <Text className={sessionHeaderSectionLabel} as="div">History</Text>
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
