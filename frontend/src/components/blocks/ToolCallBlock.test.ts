import { describe, expect, it } from "vitest";
import type { ContentBlock } from "../../generated/ship";
import { collapsedSummary } from "./toolPayload";

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

// r[verify acp.content-blocks]
// r[verify ui.block.tool-call.collapsed-default]
describe("collapsedSummary", () => {
  it("shows the structured file target for read tool calls", () => {
    const result = collapsedSummary(
      toolCallBlock({
        kind: { tag: "Read" },
        target: {
          tag: "File",
          path: "/repo/src/auth.rs",
          display_path: "src/auth.rs",
          line: null,
        },
      }),
    );
    expect(result).toBe("src/auth.rs");
  });

  it("shows diff stats from the structured diff content", () => {
    const result = collapsedSummary(
      toolCallBlock({
        kind: { tag: "Edit" },
        target: {
          tag: "File",
          path: "/repo/src/auth.rs",
          display_path: "src/auth.rs",
          line: null,
        },
        content: [
          {
            tag: "Diff",
            path: "/repo/src/auth.rs",
            display_path: "src/auth.rs",
            unified_diff:
              "--- a/src/auth.rs\n+++ b/src/auth.rs\n@@ -1,3 +1,3 @@\n fn validate() {\n-    old_check();\n+    check_token();\n }\n",
          },
        ],
      }),
    );
    expect(result).toBe("src/auth.rs  +1 -1");
  });

  // r[verify acp.terminals]
  // r[verify ui.block.tool-call.terminal]
  it("shows the structured command target for terminal tool calls", () => {
    const result = collapsedSummary(
      toolCallBlock({
        kind: { tag: "Execute" },
        target: {
          tag: "Command",
          command: "cargo nextest run -p ship-core",
          cwd: "/repo",
          display_cwd: ".",
        },
      }),
    );
    expect(result).toBe("cargo nextest run -p ship-core");
  });

  // r[verify ui.block.tool-call.search]
  it("shows the structured search query for search tool calls", () => {
    const result = collapsedSummary(
      toolCallBlock({
        kind: { tag: "Search" },
        target: {
          tag: "Search",
          query: "AwaitingPermission",
          path: "/repo/frontend/src",
          display_path: "frontend/src",
          glob: null,
        },
      }),
    );
    expect(result).toBe("AwaitingPermission");
  });

  it("falls back to the legacy arguments for older persisted blocks", () => {
    const result = collapsedSummary(
      toolCallBlock({
        tool_name: "Read",
        arguments: '{"path":"src/lib.rs"}',
      }),
    );
    expect(result).toBe("src/lib.rs");
  });
});
