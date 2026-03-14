import { describe, expect, it } from "vitest";
import { fixMarkdownBackticks } from "./fixMarkdownBackticks";

describe("fixMarkdownBackticks", () => {
  it("rewrites a single escaped backtick in a code span", () => {
    const input = "`foo(\\` bar)`";
    const output = fixMarkdownBackticks(input);
    expect(output).toBe("`` foo(` bar) ``");
  });

  it("rewrites multiple escaped backticks in one code span", () => {
    const input = "`a\\`b\\`c`";
    const output = fixMarkdownBackticks(input);
    expect(output).toBe("`` a`b`c ``");
  });

  it("passes through code spans without escaped backticks", () => {
    const input = "`normal code`";
    expect(fixMarkdownBackticks(input)).toBe("`normal code`");
  });

  it("leaves text with no code spans unchanged", () => {
    const input = "just some regular text without backticks";
    expect(fixMarkdownBackticks(input)).toBe(input);
  });

  it("does not touch double-backtick code spans", () => {
    const input = "`` already double ``";
    expect(fixMarkdownBackticks(input)).toBe(input);
  });

  it("handles escaped backtick at the start of the span", () => {
    const input = "`\\`foo`";
    const output = fixMarkdownBackticks(input);
    expect(output).toBe("`` `foo ``");
  });

  it("handles escaped backtick at the end of the span", () => {
    const input = "`foo\\``";
    const output = fixMarkdownBackticks(input);
    expect(output).toBe("`` foo` ``");
  });

  it("preserves surrounding text", () => {
    const input = "before `foo(\\` bar)` after";
    const output = fixMarkdownBackticks(input);
    expect(output).toBe("before `` foo(` bar) `` after");
  });
});
