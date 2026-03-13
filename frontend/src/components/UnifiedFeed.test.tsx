import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { ContentBlock } from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import { renderWithTheme } from "../test/render";
import { UnifiedFeed } from "./UnifiedFeed";

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

function renderFeed(blocks: BlockEntry[]) {
  return renderWithTheme(
    <UnifiedFeed
      sessionId="session-1"
      captain={null}
      mate={null}
      blocks={blocks}
      startupState={null}
      taskStatus={null}
      taskCompletedDuration={null}
    />,
  );
}

beforeEach(() => {
  if (!HTMLElement.prototype.scrollTo) {
    HTMLElement.prototype.scrollTo = () => {};
  }
});

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("UnifiedFeed task recap", () => {
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

    fireEvent.click(screen.getByRole("button", { name: /abc1234 introduce phase boundary/i }));

    const diff = screen.getByTestId("task-recap-diff");

    expect(diff).toHaveAttribute("data-diff-flow", "inline");
    expect(diff).toHaveTextContent("+++ b/src/feed.tsx");
    expect(diff).toHaveTextContent("+export function Boundary() {}");
    expect(diff).not.toHaveStyle({ maxHeight: "16rem" });
  });
});
