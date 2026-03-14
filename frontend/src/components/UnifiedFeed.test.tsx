import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { encode } from "gpt-tokenizer";
import type { AgentSnapshot, ContentBlock } from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import {
  feedRowAnimate,
  taskRecapBoundaryAccepted,
  taskRecapBoundaryError,
  taskRecapBoundaryNeutral,
} from "../styles/session-view.css";
import { renderWithTheme } from "../test/render";
import { UnifiedFeed, resetUnifiedFeedAnimationBaselinesForTest } from "./UnifiedFeed";

function makeTaskRecapBlock(): Extract<ContentBlock, { tag: "TaskRecap" }> {
  return {
    tag: "TaskRecap",
    commits: [
      {
        hash: "abc1234",
        subject: "Introduce phase boundary",
        diff: [
          "diff --git a/src/feed.tsx b/src/feed.tsx",
          "--- a/src/feed.tsx",
          "+++ b/src/feed.tsx",
          "@@ -1,2 +1,3 @@",
          " export function Feed() {}",
          "+export function Boundary() {}",
        ].join("\n"),
      },
    ],
    stats: {
      files_changed: 2,
      insertions: 12,
      deletions: 3,
    },
  };
}

function makeWorkflowMilestoneBlock(
  input:
    | Extract<ContentBlock, { tag: "WorkflowMilestone" }>["kind"]["tag"]
    | Partial<Extract<ContentBlock, { tag: "WorkflowMilestone" }>> = "StepCommitted",
): Extract<ContentBlock, { tag: "WorkflowMilestone" }> {
  if (typeof input === "string") {
    const isRebaseConflict = input === "RebaseConflict";
    return {
      tag: "WorkflowMilestone",
      kind: { tag: input },
      title: isRebaseConflict ? "Rebase conflict" : "Checkpoint committed",
      summary: isRebaseConflict
        ? "The branch could not be rebased automatically."
        : "Completed step 1: Set up types",
      items: isRebaseConflict
        ? ["Resolve conflicts in frontend/src/components/UnifiedFeed.tsx"]
        : ["Commit: abc1234", "Diff: 1 file changed, 1 insertion(+)"],
      commits: [],
      stats: null,
    };
  }

  return {
    tag: "WorkflowMilestone",
    kind: { tag: "StepCommitted" },
    title: "Checkpoint committed",
    summary: "Completed step 1: Set up types",
    items: ["Commit: abc1234", "Diff: 1 file changed, 1 insertion(+)"],
    commits: [],
    stats: null,
    ...input,
  };
}

function makeTextBlock(
  text: string,
  source: Extract<ContentBlock, { tag: "Text" }>["source"] = { tag: "AgentMessage" },
): Extract<ContentBlock, { tag: "Text" }> {
  return {
    tag: "Text",
    text,
    source,
  };
}

function makeTextEntry(
  blockId: string,
  text: string,
  roleOrOptions:
    | BlockEntry["role"]
    | {
      role?: BlockEntry["role"];
      source?: Extract<ContentBlock, { tag: "Text" }>["source"];
      timestamp?: string;
    } = {},
  sourceArg: Extract<ContentBlock, { tag: "Text" }>["source"] = { tag: "AgentMessage" },
  timestampArg = "2026-03-13T10:00:00Z",
): BlockEntry {
  let role: BlockEntry["role"] = { tag: "Captain" };
  let source: Extract<ContentBlock, { tag: "Text" }>["source"] = { tag: "AgentMessage" };
  let timestamp = timestampArg;

  if ("tag" in roleOrOptions) {
    role = roleOrOptions;
    source = sourceArg;
  } else {
    role = roleOrOptions.role ?? role;
    source = roleOrOptions.source ?? source;
    timestamp = roleOrOptions.timestamp ?? timestamp;
  }

  return {
    blockId,
    role,
    block: makeTextBlock(text, source),
    timestamp,
  };
}

function feedUi(
  blocks: BlockEntry[],
  {
    loading = false,
    sessionId = "session-1",
    captain = null,
    mate = null,
    captainTurnStartedAt = null,
    mateTurnStartedAt = null,
  }: {
    loading?: boolean;
    sessionId?: string;
    captain?: AgentSnapshot | null;
    mate?: AgentSnapshot | null;
    captainTurnStartedAt?: string | null;
    mateTurnStartedAt?: string | null;
  } = {},
) {
  return (
    <UnifiedFeed
      sessionId={sessionId}
      captain={captain}
      mate={mate}
      blocks={blocks}
      startupState={null}
      taskCompletedDuration={null}
      captainTurnStartedAt={captainTurnStartedAt}
      mateTurnStartedAt={mateTurnStartedAt}
      loading={loading}
    />
  );
}

function renderFeed(
  blocks: BlockEntry[],
  options: {
    loading?: boolean;
    sessionId?: string;
    captain?: AgentSnapshot | null;
    mate?: AgentSnapshot | null;
    captainTurnStartedAt?: string | null;
    mateTurnStartedAt?: string | null;
  } = {},
) {
  return renderWithTheme(feedUi(blocks, options));
}

function makeAgent(role: "Captain" | "Mate"): AgentSnapshot {
  return {
    role: { tag: role },
    kind: { tag: role === "Captain" ? "Claude" : "Codex" },
    state: { tag: "Working", plan: null, activity: null },
    context_remaining_percent: 80,
    preset_id: null,
    provider: null,
    model_id: null,
    available_models: [],
    effort_config_id: null,
    effort_value_id: null,
    available_effort_values: [],
  };
}

function makeToolCallEntry(
  blockId: string,
  timestamp: string,
  overrides: Partial<Extract<ContentBlock, { tag: "ToolCall" }>> = {},
): BlockEntry {
  return {
    blockId,
    role: { tag: "Captain" },
    timestamp,
    block: {
      tag: "ToolCall",
      tool_call_id: null,
      tool_name: "commentary.exec_command",
      arguments: "{\"cmd\":\"echo hi\"}",
      kind: null,
      target: null,
      raw_input: { cmd: "echo hi" } as any,
      raw_output: { stdout: "done" } as any,
      locations: [],
      status: { tag: "Success" },
      content: [
        { tag: "Text", text: "tool log" },
        {
          tag: "Terminal",
          terminal_id: "t1",
          snapshot: {
            output: "terminal output",
            truncated: false,
            exit: { exit_code: 0, signal: null },
          },
        },
      ],
      error: null,
      ...overrides,
    },
  };
}

beforeEach(() => {
  resetUnifiedFeedAnimationBaselinesForTest();
  if (!HTMLElement.prototype.scrollTo) {
    HTMLElement.prototype.scrollTo = () => {};
  }
});

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("UnifiedFeed", () => {
  // r[verify view.session]
  it("renders accepted work as a phase-break boundary band instead of a muted system note", () => {
    renderFeed([
      {
        blockId: "recap-1",
        role: { tag: "Captain" },
        block: makeTaskRecapBlock(),
        timestamp: "2026-03-13T10:00:00Z",
      },
    ]);

    const boundary = screen.getByTestId("task-recap-boundary");

    expect(boundary).toHaveAttribute("data-feed-boundary", "phase-break");
    expect(boundary).toHaveAttribute("data-phase-break-tone", "accepted");
    expect(boundary).toHaveClass(taskRecapBoundaryAccepted);
    expect(boundary).toHaveTextContent("Phase break");
    expect(boundary).toHaveTextContent("Task complete");
    expect(boundary).toHaveTextContent("+12");
    expect(boundary).toHaveTextContent("−3");
    expect(boundary).toHaveTextContent("2 files");
    expect(screen.queryByText("Work accepted")).not.toBeInTheDocument();
  });

  // r[verify view.session]
  it("expands commit diffs inline beneath the boundary without the old max-height scroller", () => {
    renderFeed([
      {
        blockId: "recap-1",
        role: { tag: "Captain" },
        block: makeTaskRecapBlock(),
        timestamp: "2026-03-13T10:00:00Z",
      },
    ]);

    fireEvent.click(screen.getByRole("button", { name: /introduce phase boundary/i }));

    const diff = screen.getByTestId("task-recap-diff");

    expect(diff).toHaveAttribute("data-diff-flow", "inline");
    expect(diff).toHaveTextContent("+++ b/src/feed.tsx");
    expect(diff).toHaveTextContent("+export function Boundary() {}");
    expect(diff).not.toHaveStyle({ maxHeight: "16rem" });
  });

  it("renders workflow milestones as generic phase-break boundaries", () => {
    renderFeed([
      {
        blockId: "milestone-1",
        role: { tag: "Captain" },
        block: makeWorkflowMilestoneBlock(),
        timestamp: "2026-03-13T10:00:00Z",
      },
    ]);

    const boundary = screen.getByTestId("workflow-milestone-boundary");

    expect(boundary).toHaveAttribute("data-feed-boundary", "phase-break");
    expect(boundary).toHaveAttribute("data-phase-break-kind", "StepCommitted");
    expect(boundary).toHaveAttribute("data-phase-break-tone", "neutral");
    expect(boundary).toHaveClass(taskRecapBoundaryNeutral);
    expect(boundary).toHaveTextContent("Phase break");
    expect(boundary).toHaveTextContent("Checkpoint committed");

    // StepCommitted starts collapsed — expand it to see summary and items
    fireEvent.click(boundary);

    expect(boundary).toHaveTextContent("Completed step 1: Set up types");
    expect(boundary).toHaveTextContent("Commit: abc1234");
    expect(boundary).toHaveTextContent("Diff: 1 file changed, 1 insertion(+)");
  });

  it("renders rebase-conflict workflow milestones with an error phase-break tone", () => {
    renderFeed([
      {
        blockId: "milestone-rebase-conflict",
        role: { tag: "Captain" },
        block: makeWorkflowMilestoneBlock("RebaseConflict"),
        timestamp: "2026-03-13T10:00:00Z",
      },
    ]);

    const boundary = screen.getByTestId("workflow-milestone-boundary");

    expect(boundary).toHaveAttribute("data-phase-break-kind", "RebaseConflict");
    expect(boundary).toHaveAttribute("data-phase-break-tone", "error");
    expect(boundary).toHaveClass(taskRecapBoundaryError);
    expect(boundary).toHaveTextContent("Rebase conflict");

    // RebaseConflict starts collapsed — expand it to see the summary
    fireEvent.click(boundary);

    expect(boundary).toHaveTextContent("The branch could not be rebased automatically.");
  });

  it("marks diff-like workflow milestone lines with the diff style hook", () => {
    renderFeed([
      {
        blockId: "milestone-1",
        role: { tag: "Captain" },
        block: makeWorkflowMilestoneBlock(),
        timestamp: "2026-03-13T10:00:00Z",
      },
    ]);

    // StepCommitted starts collapsed — expand it to see items
    fireEvent.click(screen.getByTestId("workflow-milestone-boundary"));

    const commitItem = screen.getByText("Commit: abc1234").closest('[data-testid="workflow-milestone-item"]');
    const diffItem = screen
      .getByText("Diff: 1 file changed, 1 insertion(+)")
      .closest('[data-testid="workflow-milestone-item"]');

    expect(commitItem).toHaveAttribute("data-item-style", "text");
    expect(diffItem).toHaveAttribute("data-item-style", "diff");
  });

  it("deduplicates repeated workflow milestone items while preserving order", () => {
    renderFeed([
      {
        blockId: "milestone-1",
        role: { tag: "Captain" },
        block: makeWorkflowMilestoneBlock({
          items: [
            "Commit: abc1234",
            "Diff: 1 file changed, 1 insertion(+)",
            "Diff: 1 file changed, 1 insertion(+)",
            "Commit: abc1234",
            "Follow-up note",
          ],
        }),
        timestamp: "2026-03-13T10:00:00Z",
      },
    ]);

    // StepCommitted starts collapsed — expand it to see items
    fireEvent.click(screen.getByTestId("workflow-milestone-boundary"));

    const milestoneItems = screen.getAllByTestId("workflow-milestone-item");

    expect(milestoneItems).toHaveLength(3);
    expect(milestoneItems[0]).toHaveTextContent("Commit: abc1234");
    expect(milestoneItems[1]).toHaveTextContent("Diff: 1 file changed, 1 insertion(+)");
    expect(milestoneItems[2]).toHaveTextContent("Follow-up note");
    expect(screen.getAllByText("Diff: 1 file changed, 1 insertion(+)")).toHaveLength(1);
  });

  it("renders mate updates as centered synthetic entries without raw XML", () => {
    renderFeed([
      makeTextEntry(
        "mate-update-1",
        "<mate-update>\nNeed a decision on the parser fallback.\n</mate-update>",
        { source: { tag: "Human" } },
      ),
    ]);

    const synthetic = screen.getByTestId("synthetic-human-text");

    expect(synthetic).toHaveAttribute("data-synthetic-kind", "mate-update");
    expect(screen.getByText("Mate update")).toBeInTheDocument();
    expect(screen.getByText("Need a decision on the parser fallback.")).toBeInTheDocument();
    expect(screen.queryByText(/<mate-update>/)).not.toBeInTheDocument();
  });

  it("collapses generic system notifications to a concise synthetic label", () => {
    renderFeed([
      makeTextEntry(
        "system-notification-1",
        [
          "<system-notification>",
          "You are the mate — an implementation-focused engineer.",
          "Write commit messages that describe what changed and why.",
          "</system-notification>",
        ].join("\n"),
        { source: { tag: "Human" } },
      ),
    ]);

    const synthetic = screen.getByTestId("synthetic-human-text");

    expect(synthetic).toHaveAttribute("data-synthetic-kind", "system-notification");
    expect(screen.getByText("System notification")).toBeInTheDocument();
    expect(screen.queryByText(/implementation-focused engineer/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Write commit messages that describe what changed and why/i)).not.toBeInTheDocument();
  });

  it("preserves mate activity summaries even when wrapped in a system notification", () => {
    renderFeed([
      makeTextEntry(
        "mate-summary-1",
        [
          "<system-notification>",
          "<mate-activity-summary>",
          "Refactored the parser and added focused feed tests.",
          "</mate-activity-summary>",
          "",
          "The mate's recent activity is summarized above. If something needs correction, use captain_steer.",
          "</system-notification>",
        ].join("\n"),
        { source: { tag: "Human" } },
      ),
    ]);

    expect(screen.getByText("Refactored the parser and added focused feed tests.")).toBeInTheDocument();
    expect(screen.queryByText("System notification")).not.toBeInTheDocument();
    expect(screen.queryByText(/The mate's recent activity is summarized above/i)).not.toBeInTheDocument();
  });

  it("does not animate replayed historical blocks while the feed is still loading", () => {
    const replayedBlocks = [
      makeTextEntry("history-1", "Historical block one"),
      makeTextEntry("history-2", "Historical block two"),
    ];
    const { container, rerender } = renderFeed([], { loading: true });

    expect(container.querySelectorAll(`.${feedRowAnimate}`)).toHaveLength(0);

    rerender(feedUi(replayedBlocks, { loading: true }));

    expect(screen.getByText("Historical block one")).toBeInTheDocument();
    expect(screen.getByText("Historical block two")).toBeInTheDocument();
    expect(container.querySelectorAll(`.${feedRowAnimate}`)).toHaveLength(0);

    rerender(feedUi(replayedBlocks));

    expect(container.querySelectorAll(`.${feedRowAnimate}`)).toHaveLength(0);
  });

  it("keeps existing history quiet after remounting the same session feed", () => {
    const sessionId = "session-remount";
    const historicalBlocks = [
      makeTextEntry("history-1", "Historical block one"),
      makeTextEntry("history-2", "Historical block two"),
    ];
    const liveBlock = makeTextEntry("live-1", "Fresh live block");
    const firstMount = renderFeed([], { loading: true, sessionId });

    expect(firstMount.container.querySelectorAll(`.${feedRowAnimate}`)).toHaveLength(0);

    firstMount.rerender(feedUi(historicalBlocks, { loading: true, sessionId }));
    firstMount.rerender(feedUi(historicalBlocks, { sessionId }));

    expect(screen.getByText("Historical block one")).toBeInTheDocument();
    expect(screen.getByText("Historical block two")).toBeInTheDocument();
    expect(firstMount.container.querySelectorAll(`.${feedRowAnimate}`)).toHaveLength(0);

    firstMount.unmount();

    const secondMount = renderWithTheme(feedUi(historicalBlocks, { sessionId }));

    expect(secondMount.container.querySelectorAll(`.${feedRowAnimate}`)).toHaveLength(0);

    secondMount.rerender(feedUi([...historicalBlocks, liveBlock], { sessionId }));

    const animatedWrappers = Array.from(secondMount.container.querySelectorAll(`.${feedRowAnimate}`));
    expect(animatedWrappers).toHaveLength(1);
    expect(animatedWrappers[0]).toHaveTextContent("Fresh live block");
  });

  it("animates only blocks appended after the feed becomes live", () => {
    const replayedBlocks = [makeTextEntry("history-1", "Historical block")];
    const liveBlock = makeTextEntry("live-1", "Fresh live block");
    const { container, rerender } = renderFeed([], { loading: true });

    rerender(feedUi(replayedBlocks, { loading: true }));
    rerender(feedUi(replayedBlocks));

    expect(container.querySelectorAll(`.${feedRowAnimate}`)).toHaveLength(0);

    rerender(feedUi([...replayedBlocks, liveBlock]));

    const animatedWrappers = Array.from(container.querySelectorAll(`.${feedRowAnimate}`));
    expect(animatedWrappers).toHaveLength(1);
    expect(animatedWrappers[0]).toHaveTextContent("Fresh live block");
  });

  it("counts the full current turn including agent messages while excluding relayed human and steer text", () => {
    const captain = makeAgent("Captain");
    const blocks = [
      makeTextEntry("before", "old turn", { tag: "Captain" }, { tag: "AgentThought" }, "2026-03-13T09:59:00Z"),
      makeTextEntry("msg", "visible reply", { tag: "Captain" }, { tag: "AgentMessage" }, "2026-03-13T10:00:00Z"),
      makeTextEntry("steer", "relayed steer", { tag: "Captain" }, { tag: "Steer" }, "2026-03-13T10:01:00Z"),
      makeTextEntry("human", "relayed human", { tag: "Captain" }, { tag: "Human" }, "2026-03-13T10:02:00Z"),
      makeTextEntry("thought", "internal thought", { tag: "Captain" }, { tag: "AgentThought" }, "2026-03-13T10:03:00Z"),
      makeToolCallEntry("tool", "2026-03-13T10:04:00Z"),
    ];

    renderFeed(blocks, {
      captain,
      captainTurnStartedAt: "2026-03-13T10:00:00Z",
    });

    const expectedTokens = [
      "visible reply",
      "internal thought",
      "{\"cmd\":\"echo hi\"}",
      JSON.stringify({ stdout: "done" }),
      "tool log",
      "terminal output",
    ].reduce((sum, text) => sum + encode(text).length, 0);

    expect(screen.getByText(`${expectedTokens} tokens`)).toBeInTheDocument();
    expect(screen.getByText("1✓")).toBeInTheDocument();
  });

  it("counts the entire current turn even when it extends past the 80-block render window", () => {
    const captain = makeAgent("Captain");
    const blocks: BlockEntry[] = [
      makeTextEntry("turn-start", "start marker", { tag: "Captain" }, { tag: "AgentMessage" }, "2026-03-13T10:00:00Z"),
    ];
    for (let i = 0; i < 81; i++) {
      const hour = String(10 + Math.floor((i + 1) / 60)).padStart(2, "0");
      const minute = String((i + 1) % 60).padStart(2, "0");
      blocks.push(
        makeTextEntry(
          `filler-${i}`,
          `filler ${i}`,
          { tag: "Captain" },
          { tag: "AgentThought" },
          `2026-03-13T${hour}:${minute}:00Z`,
        ),
      );
    }

    renderFeed(blocks, {
      captain,
      captainTurnStartedAt: "2026-03-13T10:00:00Z",
    });

    const expectedTokens =
      encode("start marker").length +
      Array.from({ length: 81 }, (_, i) => encode(`filler ${i}`).length).reduce((a, b) => a + b, 0);

    expect(screen.getByText(`Showing last 80 of ${blocks.length} blocks`)).toBeInTheDocument();
    expect(screen.getByText(`${expectedTokens} tokens`)).toBeInTheDocument();
  });
});
