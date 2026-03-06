import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AgentSnapshot } from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import { renderWithTheme } from "../test/render";
import { AgentPanel } from "./AgentPanel";
import { agentPanelScrollArea, eventStream, stickyPlan } from "../styles/session-view.css";

vi.mock("../api/client", () => ({
  shipClient: Promise.resolve({
    resolvePermission: async () => undefined,
  }),
}));

const agent: AgentSnapshot = {
  role: { tag: "Captain" },
  kind: { tag: "Codex" },
  state: { tag: "Working", plan: null, activity: null },
  context_remaining_percent: 88,
};

const blocks: BlockEntry[] = [
  { blockId: "text-1", role: { tag: "Captain" }, block: { tag: "Text", text: "Feed update one." } },
  {
    blockId: "plan-1",
    role: { tag: "Captain" },
    block: {
      tag: "PlanUpdate",
      steps: [
        {
          description: "Outdated plan step",
          priority: { tag: "Low" },
          status: { tag: "Pending" },
        },
      ],
    },
  },
  { blockId: "text-2", role: { tag: "Captain" }, block: { tag: "Text", text: "Feed update two." } },
  {
    blockId: "plan-2",
    role: { tag: "Captain" },
    block: {
      tag: "PlanUpdate",
      steps: [
        {
          description: "Queue the UI patch",
          priority: { tag: "High" },
          status: { tag: "Pending" },
        },
        {
          description: "Render the sticky plan",
          priority: { tag: "Medium" },
          status: { tag: "InProgress" },
        },
        {
          description: "Ship the component tests",
          priority: { tag: "Low" },
          status: { tag: "Completed" },
        },
        { description: "Handle regressions", priority: { tag: "High" }, status: { tag: "Failed" } },
      ],
    },
  },
];

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("AgentPanel plan rendering", () => {
  // r[verify ui.block.plan.layout]
  // r[verify ui.block.plan.position]
  // r[verify ui.block.plan.filtering]
  it("shows only the latest plan above the feed and filters plan updates from the chronological stream", () => {
    const { container } = renderWithTheme(
      <AgentPanel sessionId="session-1" agent={agent} blocks={blocks} loading={false} />,
    );

    const planList = screen.getByRole("list");
    expect(planList.tagName).toBe("OL");
    expect(within(planList).getByText("Queue the UI patch")).toBeInTheDocument();
    expect(within(planList).getByText("Render the sticky plan")).toBeInTheDocument();
    expect(within(planList).getByText("Ship the component tests")).toBeInTheDocument();
    expect(within(planList).getByText("Handle regressions")).toBeInTheDocument();
    expect(screen.queryByText("Outdated plan step")).not.toBeInTheDocument();

    const items = within(planList).getAllByRole("listitem");
    expect(items).toHaveLength(4);
    expect(within(items[0]).getByText("high")).toBeInTheDocument();
    expect(within(items[0]).getByLabelText("Pending")).toBeInTheDocument();
    expect(within(items[1]).getByText("medium")).toBeInTheDocument();
    expect(within(items[1]).getByLabelText("In progress")).toBeInTheDocument();
    expect(within(items[2]).getByText("low")).toBeInTheDocument();
    expect(within(items[2]).getByLabelText("Completed")).toBeInTheDocument();
    expect(within(items[3]).getByText("high")).toBeInTheDocument();
    expect(within(items[3]).getByLabelText("Failed")).toBeInTheDocument();

    const scrollArea = container.querySelector<HTMLElement>(`.${agentPanelScrollArea}`);
    const stickyPlanArea = container.querySelector<HTMLElement>(`.${stickyPlan}`);
    const feed = container.querySelector<HTMLElement>(`.${eventStream}`);

    expect(scrollArea).toContainElement(stickyPlanArea);
    expect(scrollArea).toContainElement(feed);
    expect(stickyPlanArea?.compareDocumentPosition(feed ?? document.body) ?? 0).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );

    expect(feed).toHaveTextContent("Feed update one.");
    expect(feed).toHaveTextContent("Feed update two.");
    expect(feed).not.toHaveTextContent("Queue the UI patch");
    expect(feed).not.toHaveTextContent("Render the sticky plan");
  });
});
