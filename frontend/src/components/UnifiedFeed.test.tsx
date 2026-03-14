import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { ContentBlock } from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import { feedRowAnimate } from "../styles/session-view.css";
import { renderWithTheme } from "../test/render";
import { UnifiedFeed, resetUnifiedFeedAnimationStateForTests } from "./UnifiedFeed";

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

function makeTextBlock(text: string): Extract<ContentBlock, { tag: "Text" }> {
  return {
    tag: "Text",
    text,
    source: { tag: "AgentMessage" },
  };
}

function makeTextEntry(blockId: string, text: string): BlockEntry {
  return {
    blockId,
    role: { tag: "Captain" },
    block: makeTextBlock(text),
    timestamp: "2026-03-13T10:00:00Z",
  };
}

function feedUi(
  blocks: BlockEntry[],
  { loading = false, sessionId = "session-1" }: { loading?: boolean; sessionId?: string } = {},
) {
  return (
    <UnifiedFeed
      sessionId={sessionId}
      captain={null}
      mate={null}
      blocks={blocks}
      startupState={null}
      taskCompletedDuration={null}
      loading={loading}
    />
  );
}

function renderFeed(
  blocks: BlockEntry[],
  options: { loading?: boolean; sessionId?: string } = {},
) {
  return renderWithTheme(feedUi(blocks, options));
}

beforeEach(() => {
  resetUnifiedFeedAnimationStateForTests();
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

  it("does not animate the initial population of a freshly mounted session feed", () => {
    const sessionId = "session-remount";
    const historicalBlocks = [
      makeTextEntry("history-1", "Historical block one"),
      makeTextEntry("history-2", "Historical block two"),
    ];
    const firstMount = renderFeed([], { sessionId });

    expect(firstMount.container.querySelectorAll(`.${feedRowAnimate}`)).toHaveLength(0);

    firstMount.rerender(feedUi(historicalBlocks, { sessionId }));

    expect(screen.getByText("Historical block one")).toBeInTheDocument();
    expect(screen.getByText("Historical block two")).toBeInTheDocument();
    expect(firstMount.container.querySelectorAll(`.${feedRowAnimate}`)).toHaveLength(0);

    firstMount.unmount();

    const secondMount = renderWithTheme(feedUi(historicalBlocks, { sessionId }));

    expect(secondMount.container.querySelectorAll(`.${feedRowAnimate}`)).toHaveLength(0);
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
});
