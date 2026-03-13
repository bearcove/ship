import { useId, useMemo, useState } from "react";
import { Badge, Code, DropdownMenu, Flex, IconButton, Spinner, Text } from "@radix-ui/themes";
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
  feedBubble,
  planStepRow,
  planStepText,
  sessionHeaderAgentLabel,
  sessionHeaderAgentRow,
  sessionHeaderBranchMeta,
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
  taskDescriptionRoot,
} from "../styles/session-view.css";
import { NewSessionDialog } from "../pages/SessionListPage";

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
            <CaretDown size={11} className={sessionHeaderCaret} />
          ) : (
            <CaretRight size={11} className={sessionHeaderCaret} />
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

            {/* Agents */}
            {(captain ?? mate) && (
              <div>
                <Text className={sessionHeaderSectionLabel} as="div">
                  Agents
                </Text>
                <Flex direction="column" gap="2">
                  {captain && (
                    <div className={sessionHeaderAgentRow}>
                      <Text size="1" color="gray" className={sessionHeaderAgentLabel}>
                        Captain
                      </Text>
                      <AgentModelPicker sessionId={sessionId} agent={captain} />
                      <AgentEffortPicker sessionId={sessionId} agent={captain} />
                    </div>
                  )}
                  {mate && (
                    <div className={sessionHeaderAgentRow}>
                      <Text size="1" color="gray" className={sessionHeaderAgentLabel}>
                        Mate
                      </Text>
                      <AgentModelPicker sessionId={sessionId} agent={mate} />
                      <AgentEffortPicker sessionId={sessionId} agent={mate} />
                    </div>
                  )}
                </Flex>
              </div>
            )}

            {/* Branch + diff */}
            <div>
              <Text className={sessionHeaderSectionLabel} as="div">
                Branch
              </Text>
              <div className={sessionHeaderBranchMeta}>
                <Code variant="ghost" size="1">
                  {branchName}
                </Code>
                {diffStats && (
                  <>
                    <Text size="1" color="gray">
                      ·
                    </Text>
                    <Text size="1" className={sessionHeaderDiffAdd}>
                      +{String(diffStats.lines_added)}
                    </Text>
                    <Text size="1" className={sessionHeaderDiffRemove}>
                      -{String(diffStats.lines_removed)}
                    </Text>
                    {diffStats.files_changed > 0n && (
                      <Text size="1" color="gray">
                        · {String(diffStats.files_changed)} files
                      </Text>
                    )}
                  </>
                )}
              </div>
            </div>

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
