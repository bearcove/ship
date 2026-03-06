import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { TextBlock } from "./TextBlock";
import { renderWithTheme } from "../../test/render";

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("TextBlock", () => {
  // r[verify ui.block.text]
  it("renders markdown, inline code, and highlighted fenced code blocks", async () => {
    const { container } = renderWithTheme(
      <TextBlock
        block={{
          tag: "Text",
          text: "Paragraph with `shipctl`.\n\n```ts\nconst answer = 42;\n```",
        }}
      />,
    );

    expect(screen.getByText(/Paragraph with/).closest("p")).toBeInTheDocument();
    expect(screen.getByText("shipctl").tagName).toBe("CODE");
    expect(container).toHaveTextContent("const answer = 42;");

    await waitFor(() => {
      expect(container.querySelector(".shiki")).toBeInTheDocument();
    });
  });
});
