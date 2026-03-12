import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { ContentBlock } from "../../generated/ship";
import { renderWithTheme } from "../../test/render";
import { PlanUpdateBlock } from "./PlanUpdateBlock";

type PlanUpdateBlockType = Extract<ContentBlock, { tag: "PlanUpdate" }>;

const block = {
  tag: "PlanUpdate",
  steps: [
    {
      title: "Queue the UI patch",
      description: "Queue the UI patch for review",
      status: { tag: "Pending" },
    },
    {
      title: "Render the sticky plan",
      description: "Render the sticky plan component",
      status: { tag: "InProgress" },
    },
    {
      title: "Ship the component tests",
      description: "Ship the component tests to CI",
      status: { tag: "Completed" },
    },
    {
      title: "Handle regressions",
      description: "Handle regressions from the rollout",
      status: { tag: "Failed" },
    },
  ],
} satisfies PlanUpdateBlockType;

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("PlanUpdateBlock", () => {
  // r[verify ui.block.plan.layout]
  it("renders a compact ordered list with the plan steps and status icons", () => {
    renderWithTheme(<PlanUpdateBlock block={block} />);

    const planList = screen.getByRole("list");
    expect(planList.tagName).toBe("OL");
    expect(planList.style.paddingInlineStart).toBe("var(--space-5)");

    const items = within(planList).getAllByRole("listitem");
    expect(items).toHaveLength(4);

    expect(items[0].style.fontSize).toBe("var(--font-size-1)");
    expect(items[0].style.paddingInlineStart).toBe("var(--space-1)");

    expect(within(items[0]).getByText("Queue the UI patch")).toBeInTheDocument();
    expect(within(items[0]).getByLabelText("Pending")).toBeInTheDocument();

    expect(within(items[1]).getByText("Render the sticky plan")).toBeInTheDocument();
    expect(within(items[1]).getByLabelText("In progress")).toBeInTheDocument();

    expect(within(items[2]).getByText("Ship the component tests")).toBeInTheDocument();
    expect(within(items[2]).getByLabelText("Completed")).toBeInTheDocument();

    expect(within(items[3]).getByText("Handle regressions")).toBeInTheDocument();
    expect(within(items[3]).getByLabelText("Failed")).toBeInTheDocument();
  });
});
