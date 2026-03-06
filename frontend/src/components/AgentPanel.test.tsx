import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentSnapshot } from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import { renderWithTheme } from "../test/render";
import { AgentPanel } from "./AgentPanel";
import {
  agentPanelScrollArea,
  eventStream,
  feedMessageCardAgent,
  feedMessageCardThought,
  feedMessageCardUser,
  stickyPlan,
} from "../styles/session-view.css";

const apiMocks = vi.hoisted(() => ({
  resolvePermission: vi.fn(async () => undefined),
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    resolvePermission: apiMocks.resolvePermission,
  }),
}));

const agent: AgentSnapshot = {
  role: { tag: "Captain" },
  kind: { tag: "Codex" },
  state: { tag: "Working", plan: null, activity: null },
  context_remaining_percent: 88,
};

const blocks: BlockEntry[] = [
  {
    blockId: "text-1",
    role: { tag: "Captain" },
    block: { tag: "Text", text: "Feed update one.", source: { tag: "AgentMessage" } },
  },
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
  {
    blockId: "text-2",
    role: { tag: "Captain" },
    block: { tag: "Text", text: "Feed update two.", source: { tag: "AgentMessage" } },
  },
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

beforeEach(() => {
  apiMocks.resolvePermission.mockClear();
});

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("AgentPanel plan rendering", () => {
  // r[verify view.session]
  it("renders startup progress inline in the captain feed", () => {
    renderWithTheme(
      <AgentPanel
        sessionId="session-1"
        agent={agent}
        blocks={[]}
        loading={false}
        startupState={{
          tag: "Running",
          stage: { tag: "StartingCaptain" },
          message: "Starting captain (0.4s elapsed)",
        }}
        taskStatus={null}
      />,
    );

    expect(screen.getByText("Session startup")).toBeInTheDocument();
    expect(screen.getByText("Starting captain (0.4s elapsed)")).toBeInTheDocument();
  });

  // r[verify ui.block.plan.layout]
  // r[verify ui.block.plan.position]
  // r[verify ui.block.plan.filtering]
  it("shows only the latest plan above the feed and filters plan updates from the chronological stream", () => {
    const { container } = renderWithTheme(
      <AgentPanel
        sessionId="session-1"
        agent={agent}
        blocks={blocks}
        loading={false}
        startupState={{ tag: "Ready" }}
        taskStatus={{ tag: "ReviewPending" }}
      />,
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

  it("right-aligns human messages and renders structured thought blocks distinctly", () => {
    const { container } = renderWithTheme(
      <AgentPanel
        sessionId="session-1"
        agent={agent}
        blocks={[
          {
            blockId: "agent",
            role: { tag: "Captain" },
            block: { tag: "Text", text: "Ready to help.", source: { tag: "AgentMessage" } },
          },
          {
            blockId: "human",
            role: { tag: "Captain" },
            block: {
              tag: "Text",
              text: "Can you ask your mate to patch this?",
              source: { tag: "Human" },
            },
          },
          {
            blockId: "thought",
            role: { tag: "Captain" },
            block: {
              tag: "Text",
              text: "I should inspect the codebase before delegating.",
              source: { tag: "AgentThought" },
            },
          },
        ]}
        loading={false}
        startupState={{ tag: "Ready" }}
        taskStatus={{ tag: "Assigned" }}
      />,
    );

    expect(screen.queryByText("Captain")).not.toBeInTheDocument();
    expect(screen.queryByText("You")).not.toBeInTheDocument();
    expect(screen.getByText("Can you ask your mate to patch this?")).toBeInTheDocument();
    expect(screen.getByText("Thinking")).toBeInTheDocument();

    const cards = container.querySelectorAll<HTMLElement>(
      `.${feedMessageCardAgent}, .${feedMessageCardUser}, .${feedMessageCardThought}`,
    );
    expect(cards).toHaveLength(3);
    expect(cards[0]).toHaveClass(feedMessageCardAgent);
    expect(cards[1]).toHaveClass(feedMessageCardUser);
    expect(cards[2]).toHaveClass(feedMessageCardThought);
  });

  it("does not misclassify captain text with leading whitespace as a user bubble", () => {
    const { container } = renderWithTheme(
      <AgentPanel
        sessionId="session-1"
        agent={agent}
        blocks={[
          {
            blockId: "captain-whitespace",
            role: { tag: "Captain" },
            block: { tag: "Text", text: "\nReady to help.", source: { tag: "AgentMessage" } },
          },
        ]}
        loading={false}
        startupState={{ tag: "Ready" }}
        taskStatus={{ tag: "Assigned" }}
      />,
    );

    const userCards = container.querySelectorAll<HTMLElement>(`.${feedMessageCardUser}`);
    const agentCards = container.querySelectorAll<HTMLElement>(`.${feedMessageCardAgent}`);

    expect(userCards).toHaveLength(0);
    expect(agentCards).toHaveLength(1);
  });

  // r[verify view.permission-dialog]
  it("keeps the latest unresolved permission request actionable in the feed", async () => {
    apiMocks.resolvePermission.mockClear();

    renderWithTheme(
      <AgentPanel
        sessionId="session-1"
        agent={{
          ...agent,
          state: {
            tag: "AwaitingPermission",
            request: {
              permission_id: "perm-2",
              tool_call_id: "toolu_2",
              tool_name: "Read File",
              arguments: '{"path":"src/lib.rs"}',
              description: "Read file",
              kind: { tag: "Read" },
              target: {
                tag: "File",
                path: "/repo/src/lib.rs",
                display_path: "src/lib.rs",
                line: null,
              },
              raw_input: null,
              options: [
                {
                  option_id: "allow-once",
                  label: "Allow once",
                  kind: { tag: "AllowOnce" },
                },
                {
                  option_id: "reject-once",
                  label: "Reject once",
                  kind: { tag: "RejectOnce" },
                },
              ],
            },
          },
        }}
        blocks={[
          ...blocks,
          {
            blockId: "perm-1",
            role: { tag: "Captain" },
            block: {
              tag: "Permission",
              permission_id: "perm-1",
              tool_call_id: "toolu_1",
              tool_name: "Read File",
              description: "Read file",
              arguments: '{"path":"src/old.rs"}',
              kind: { tag: "Read" },
              target: {
                tag: "File",
                path: "/repo/src/old.rs",
                display_path: "src/old.rs",
                line: null,
              },
              raw_input: null,
              options: [
                {
                  option_id: "allow-once-old",
                  label: "Allow once",
                  kind: { tag: "AllowOnce" },
                },
              ],
              resolution: null,
            },
          },
          {
            blockId: "perm-2",
            role: { tag: "Captain" },
            block: {
              tag: "Permission",
              permission_id: "perm-2",
              tool_call_id: "toolu_2",
              tool_name: "Read File",
              description: "Read file",
              arguments: '{"path":"src/lib.rs"}',
              kind: { tag: "Read" },
              target: {
                tag: "File",
                path: "/repo/src/lib.rs",
                display_path: "src/lib.rs",
                line: null,
              },
              raw_input: null,
              options: [
                {
                  option_id: "allow-once",
                  label: "Allow once",
                  kind: { tag: "AllowOnce" },
                },
                {
                  option_id: "reject-once",
                  label: "Reject once",
                  kind: { tag: "RejectOnce" },
                },
              ],
              resolution: null,
            },
          },
        ]}
        loading={false}
        startupState={{ tag: "Ready" }}
        taskStatus={{ tag: "Working" }}
      />,
    );

    const approveButtons = screen.getAllByRole("button", { name: "Approve" });
    expect(approveButtons).toHaveLength(2);
    expect(approveButtons[0]).toBeDisabled();
    expect(approveButtons[1]).toBeEnabled();

    fireEvent.click(approveButtons[1]);
    await waitFor(() => {
      expect(apiMocks.resolvePermission).toHaveBeenCalledWith("session-1", "perm-2", "allow-once");
    });
  });
});
