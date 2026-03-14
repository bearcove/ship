import { useId, useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Badge, Box, Code, Flex, Spinner, Text } from "@radix-ui/themes";
import {
  CaretDown,
  CaretRight,
  Check,
  CheckCircle,
  Circle,
  Copy,
  XCircle,
} from "@phosphor-icons/react";
import type {
  PlanStep,
  PlanStepStatus,
  TaskRecord,
  TaskStatus,
  WorktreeDiffStats,
} from "../generated/ship";
import { planStepRow, planStepText, taskDescriptionRoot } from "../styles/session-view.css";
import { MarkdownCodeBlock } from "./blocks/TextBlock";

function StepIcon({ status }: { status: PlanStepStatus }) {
  switch (status.tag) {
    case "Pending":
      return <Circle size={12} style={{ color: "var(--gray-8)", flexShrink: 0 }} />;
    case "InProgress":
      return <Spinner size="1" />;
    case "Completed":
      return (
        <CheckCircle size={12} weight="fill" style={{ color: "var(--green-9)", flexShrink: 0 }} />
      );
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

const titleMdComponents: React.ComponentProps<typeof ReactMarkdown>["components"] = {
  p: ({ children }) => <>{children}</>,
};

const mdComponents: React.ComponentProps<typeof ReactMarkdown>["components"] = {
  code({ children, className }: { children?: React.ReactNode; className?: string }) {
    const rawCode = String(children ?? "");
    const isBlock =
      Boolean(className?.startsWith("language-")) ||
      rawCode.includes("\n") ||
      rawCode.endsWith("\n");
    const code = rawCode.replace(/\n$/, "");
    if (isBlock) {
      return <MarkdownCodeBlock className={className} code={code} />;
    }
    return <Code size="1">{children}</Code>;
  },
};

function TaskListItem({
  task,
  defaultExpanded = false,
}: {
  task: TaskRecord;
  defaultExpanded?: boolean;
}) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const headerId = useId();
  const bodyId = useId();

  return (
    <Flex
      direction="column"
      style={{
        borderTop: "1px solid var(--gray-a4)",
        minWidth: 0,
      }}
    >
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
          fontFamily: "inherit",
          textAlign: "left",
          cursor: "pointer",
        }}
      >
        {expanded ? (
          <CaretDown size={11} style={{ color: "var(--gray-10)", flexShrink: 0, marginTop: 3 }} />
        ) : (
          <CaretRight size={11} style={{ color: "var(--gray-10)", flexShrink: 0, marginTop: 3 }} />
        )}
        <Flex direction="column" gap="1" style={{ minWidth: 0, flex: 1 }}>
          <Flex align="center" gap="2" wrap="wrap">
            <Text
              size="1"
              weight="medium"
              as="div"
              style={{ lineHeight: 1.35, flex: 1, minWidth: 0 }}
            >
              <ReactMarkdown remarkPlugins={[remarkGfm]} components={titleMdComponents}>
                {task.title}
              </ReactMarkdown>
            </Text>
            <TaskStatusBadge status={task.status} />
          </Flex>
        </Flex>
      </button>

      {expanded && (
        <Box
          id={bodyId}
          role="region"
          aria-labelledby={headerId}
          style={{
            paddingLeft: "calc(11px + var(--space-2))",
            paddingBottom: "var(--space-2)",
          }}
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

interface Props {
  liveTask: TaskRecord | null;
  taskHistory: TaskRecord[];
  branchName: string;
  diffStats: WorktreeDiffStats | null;
  planSteps: PlanStep[];
  matePlan: PlanStep[] | null;
}

// r[view.task-panel]
export function SessionTaskDrawer({
  liveTask,
  taskHistory,
  branchName,
  diffStats,
  planSteps,
  matePlan,
}: Props) {
  const hasActivePlan = !!matePlan && matePlan.length > 0;
  const [expanded, setExpanded] = useState(hasActivePlan);
  const [copied, setCopied] = useState(false);
  const contentId = useId();

  function handleCopyPlan() {
    if (!matePlan) return;
    const text = matePlan
      .map((step, i) => `${i + 1}. ${step.title || step.description}`)
      .join("\n");
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }
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
          fontFamily: "inherit",
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
          <Flex align="center" gap="2" style={{ minWidth: 0 }}>
            <Text
              size="2"
              weight="medium"
              data-testid="session-task-drawer-title"
              style={{ minWidth: 0, lineHeight: 1.35, flex: 1 }}
            >
              {summary}
            </Text>
            <Flex
              align="center"
              gap="1"
              data-testid="session-task-drawer-progress"
              aria-label={`Task progress: ${planSteps.filter((s) => s.status.tag === "Completed").length} of ${planSteps.length} steps done`}
              style={{ flexShrink: 0 }}
            >
              {planSteps.length > 0
                ? planSteps.map((step, index) => {
                    const complete = step.status.tag === "Completed";
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
                : null}
            </Flex>
          </Flex>
        </Flex>
      </button>

      {expanded && (
        <Box
          id={contentId}
          data-testid="session-task-drawer-content"
          style={{
            maxHeight: "28rem",
            overflowY: "auto",
            padding: "0 var(--space-3) var(--space-3)",
            display: "flex",
            flexDirection: "column",
            gap: "var(--space-3)",
          }}
        >
          {matePlan && matePlan.length > 0 && (
            <Flex direction="column" gap="1">
              <Flex align="center" justify="between">
                <Text size="1" weight="medium" color="gray">
                  Plan
                </Text>
                <button
                  type="button"
                  onClick={handleCopyPlan}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    background: "transparent",
                    border: 0,
                    padding: "2px",
                    cursor: "pointer",
                    color: copied ? "var(--green-9)" : "var(--gray-9)",
                  }}
                  aria-label="Copy plan to clipboard"
                >
                  {copied ? <Check size={13} weight="bold" /> : <Copy size={13} />}
                </button>
              </Flex>
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
                      textDecoration: step.status.tag === "Completed" ? "line-through" : undefined,
                    }}
                  >
                    {step.title || step.description}
                  </Text>
                </Flex>
              ))}
            </Flex>
          )}
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

          <Flex direction="column">
            {liveTask ? (
              <TaskListItem task={liveTask} defaultExpanded={true} />
            ) : (
              <Text size="1" color="gray" style={{ paddingTop: "var(--space-2)" }}>
                No active task
              </Text>
            )}
            {history.length > 0 && (
              <Text
                size="1"
                color="gray"
                style={{ padding: "var(--space-1) 0 var(--space-1) calc(11px + var(--space-2))" }}
              >
                {history.length} previous {history.length === 1 ? "step" : "steps"}
              </Text>
            )}
            {history.map((task) => (
              <TaskListItem key={task.id} task={task} defaultExpanded={false} />
            ))}
          </Flex>
        </Box>
      )}
    </Box>
  );
}
