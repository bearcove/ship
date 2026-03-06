import { describe, it, expect } from "vitest";
import { collapsedSummary } from "./ToolCallBlock";

const noContent: never[] = [];
const noLocations: never[] = [];

// r[verify ui.block.tool-call.collapsed-default]
describe("collapsedSummary", () => {
  it("shows path for Read tool calls", () => {
    const result = collapsedSummary("Read", { path: "src/auth.rs" }, noContent, noLocations);
    expect(result).toBe("src/auth.rs");
  });

  it("shows path for Write tool calls without diff result", () => {
    const result = collapsedSummary("Write", { path: "src/lib.rs" }, noContent, noLocations);
    expect(result).toBe("src/lib.rs");
  });

  it("shows path and diff summary for Edit tool calls with diff result", () => {
    const result = collapsedSummary(
      "Edit",
      { path: "src/auth.rs" },
      [
        {
          tag: "Diff",
          path: "src/auth.rs",
          old_text: "fn validate() {\n    old_check();\n}",
          new_text: "fn validate() {\n    check_token();\n}",
        },
      ],
      noLocations,
    );
    expect(result).toBe("src/auth.rs  +1 -1");
  });

  it("shows path without diff summary when result is not a diff", () => {
    const result = collapsedSummary("Write", { path: "src/lib.rs" }, noContent, noLocations);
    expect(result).toBe("src/lib.rs");
  });

  // r[verify ui.block.tool-call.terminal]
  it("shows command for Bash tool calls", () => {
    const result = collapsedSummary(
      "Bash",
      { command: "cargo test -p roam-session 2>&1" },
      noContent,
      noLocations,
    );
    expect(result).toBe("cargo test -p roam-session 2>&1");
  });

  // r[verify ui.block.tool-call.search]
  it("shows pattern for Grep tool calls", () => {
    const result = collapsedSummary("Grep", { pattern: "reconnect" }, noContent, noLocations);
    expect(result).toBe("reconnect");
  });

  it("shows pattern for Glob tool calls", () => {
    const result = collapsedSummary("Glob", { pattern: "**/*.rs" }, noContent, noLocations);
    expect(result).toBe("**/*.rs");
  });

  it("returns empty string when no relevant arg found", () => {
    const result = collapsedSummary("UnknownTool", {}, noContent, noLocations);
    expect(result).toBe("");
  });

  it("handles non-JSON arguments gracefully", () => {
    const result = collapsedSummary("Read", {}, noContent, noLocations);
    expect(result).toBe("");
  });
});

// r[verify ui.block.tool-call.diff]
describe("diff summary counting", () => {
  it("counts added and removed lines correctly", () => {
    const result = collapsedSummary(
      "Edit",
      { path: "foo.rs" },
      [
        {
          tag: "Diff",
          path: "foo.rs",
          old_text: "removed line",
          new_text: "added line 1\nadded line 2",
        },
      ],
      noLocations,
    );
    expect(result).toBe("foo.rs  +2 -1");
  });

  it("does not count +++ and --- header lines as diff lines", () => {
    const result = collapsedSummary(
      "Edit",
      { path: "foo.rs" },
      [{ tag: "Diff", path: "foo.rs", old_text: "old", new_text: "new" }],
      noLocations,
    );
    expect(result).toBe("foo.rs  +1 -1");
  });
});
