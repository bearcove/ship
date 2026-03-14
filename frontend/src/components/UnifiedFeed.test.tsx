import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { encode } from "gpt-tokenizer";
import type { AgentSnapshot, ContentBlock } from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import { feedRowAnimate } from "../styles/session-view.css";
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

function makeTextBlock(
  text: string,
  source: Extract<ContentBlock, { tag: "Text" }>['source'] = { tag: "AgentMessage" },
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
  role: BlockEntry['role'] = { tag: "Captain" },
  source: Extract<ContentBlock, { tag: "Text" }>['source'] = { tag: "AgentMessage" },
  timestamp = "2026-03-13T10:00:00Z",
): BlockEntry {
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
      raw_input: { cmd: "echo hi" },
      raw_output: { stdout: "done" },
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
    expect(boundary).toHaveTextContent("Phase break");
    expect(boundary).toHaveTextContent("Previous task accepted");
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
