import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ErrorBlock } from "./ErrorBlock";
import { renderWithTheme } from "../../test/render";

const errorBlock = {
  tag: "Error" as const,
  message: "The agent crashed while applying the patch.",
};

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("ErrorBlock", () => {
  // r[verify ui.block.error]
  it("renders the error callout body and only shows retry while the agent is in error", () => {
    const { rerender } = renderWithTheme(
      <ErrorBlock block={errorBlock} agentState={{ tag: "Idle" }} />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(errorBlock.message);
    expect(screen.getByLabelText("Error")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();

    rerender(<ErrorBlock block={errorBlock} agentState={{ tag: "Error", message: "boom" }} />);

    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });
});
