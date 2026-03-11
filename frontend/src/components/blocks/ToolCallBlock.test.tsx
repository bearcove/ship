import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { ContentBlock } from "../../generated/ship";
import { renderWithTheme } from "../../test/render";
import { ToolCallBlock } from "./ToolCallBlock";

function toolCallBlock(
  overrides: Partial<Extract<ContentBlock, { tag: "ToolCall" }>>,
): Extract<ContentBlock, { tag: "ToolCall" }> {
  return {
    tag: "ToolCall",
    tool_call_id: "toolu_1",
    tool_name: "Tool",
    arguments: "{}",
    kind: null,
    target: null,
    raw_input: null,
    raw_output: null,
    locations: [],
    status: { tag: "Success" },
    content: [],
    error: null,
    ...overrides,
  };
}

describe("ToolCallBlock", () => {
  // r[verify ui.block.tool-call.layout]
  // r[verify ui.block.tool-call.terminal]
  it("renders structured terminal snapshots with decoded ANSI output and exit state", () => {
    renderWithTheme(
      <ToolCallBlock
        block={toolCallBlock({
          tool_name: "terminal",
          kind: { tag: "Execute" },
          target: {
            tag: "Command",
            command: "pnpm --dir frontend test",
            cwd: "/repo",
            display_cwd: ".",
          },
          status: { tag: "Failure" },
          content: [
            {
              tag: "Terminal",
              terminal_id: "term-1",
              snapshot: {
                output: "\u001b[31mfail\u001b[0m\nplain",
                exit: { exit_code: 2, signal: null },
              },
            },
          ],
        })}
      />,
    );

    expect(screen.getByText("pnpm --dir frontend test")).toBeInTheDocument();
    expect(screen.getByText("✗ Failed")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button"));

    expect(screen.getByText("$ pnpm --dir frontend test")).toBeInTheDocument();
    expect(screen.getByText("exit 2")).toBeInTheDocument();
    expect(screen.getByText("fail")).toBeInTheDocument();
    expect(screen.getByText("plain")).toBeInTheDocument();
  });

  // r[verify acp.content-blocks]
  it("keeps structured tool errors separate from markdown text content", () => {
    renderWithTheme(
      <ToolCallBlock
        block={toolCallBlock({
          tool_name: "read_file",
          kind: { tag: "Read" },
          target: {
            tag: "File",
            path: "/repo/src/lib.rs",
            display_path: "src/lib.rs",
            line: null,
          },
          status: { tag: "Failure" },
          content: [
            {
              tag: "Text",
              text: "```terminal\nnot structured terminal output\n```",
            },
          ],
          error: {
            message: "read failed",
            details: {
              tag: "Object",
              entries: [{ key: "reason", value: { tag: "String", value: "permission denied" } }],
            },
          },
        })}
      />,
    );

    fireEvent.click(screen.getByRole("button"));

    expect(screen.getByText("read failed")).toBeInTheDocument();
    expect(screen.getByText(/not structured terminal output/)).toBeInTheDocument();
    expect(screen.queryByText(/\$ /)).not.toBeInTheDocument();
  });
});
