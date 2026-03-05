import { describe, it, expect } from "vitest";
import { collapsedSummary } from "./ToolCallBlock";

// r[verify ui.block.tool-call.collapsed-default]
describe("collapsedSummary", () => {
  it("shows path for Read tool calls", () => {
    const result = collapsedSummary("Read", { path: "src/auth.rs" }, null);
    expect(result).toBe("src/auth.rs");
  });

  it("shows path for Write tool calls without diff result", () => {
    const result = collapsedSummary("Write", { path: "src/lib.rs" }, "ok");
    expect(result).toBe("src/lib.rs");
  });

  it("shows path and diff summary for Edit tool calls with diff result", () => {
    const diff = [
      "--- a/src/auth.rs",
      "+++ b/src/auth.rs",
      "@@ -10,3 +10,5 @@",
      " fn validate() {",
      "+    check_token();",
      "-    old_check();",
      " }",
    ].join("\n");
    const result = collapsedSummary("Edit", { path: "src/auth.rs" }, diff);
    expect(result).toBe("src/auth.rs  +1 -1");
  });

  it("shows path without diff summary when result is not a diff", () => {
    const result = collapsedSummary("Write", { path: "src/lib.rs" }, "Created file");
    expect(result).toBe("src/lib.rs");
  });

  // r[verify ui.block.tool-call.terminal]
  it("shows command for Bash tool calls", () => {
    const result = collapsedSummary("Bash", { command: "cargo test -p roam-session 2>&1" }, null);
    expect(result).toBe("cargo test -p roam-session 2>&1");
  });

  // r[verify ui.block.tool-call.search]
  it("shows pattern for Grep tool calls", () => {
    const result = collapsedSummary("Grep", { pattern: "reconnect" }, null);
    expect(result).toBe("reconnect");
  });

  it("shows pattern for Glob tool calls", () => {
    const result = collapsedSummary("Glob", { pattern: "**/*.rs" }, null);
    expect(result).toBe("**/*.rs");
  });

  it("returns empty string when no relevant arg found", () => {
    const result = collapsedSummary("UnknownTool", {}, null);
    expect(result).toBe("");
  });

  it("handles non-JSON arguments gracefully", () => {
    const result = collapsedSummary("Read", {}, null);
    expect(result).toBe("");
  });
});

// r[verify ui.block.tool-call.diff]
describe("diff summary counting", () => {
  it("counts added and removed lines correctly", () => {
    const diff = [
      "--- a/foo.rs",
      "+++ b/foo.rs",
      "@@ -1,4 +1,5 @@",
      " unchanged",
      "+added line 1",
      "+added line 2",
      "-removed line",
      " unchanged",
    ].join("\n");
    const result = collapsedSummary("Edit", { path: "foo.rs" }, diff);
    expect(result).toBe("foo.rs  +2 -1");
  });

  it("does not count +++ and --- header lines as diff lines", () => {
    const diff = ["--- a/foo.rs", "+++ b/foo.rs", "@@ -1,1 +1,1 @@", "+new", "-old"].join("\n");
    const result = collapsedSummary("Edit", { path: "foo.rs" }, diff);
    expect(result).toBe("foo.rs  +1 -1");
  });
});
