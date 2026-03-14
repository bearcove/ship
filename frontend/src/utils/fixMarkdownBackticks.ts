/**
 * Preprocess markdown text to fix backtick-escaped code spans that CommonMark
 * doesn't support. LLMs often write `foo(\` bar)` thinking the backslash
 * escapes the backtick, but the parser closes the code span at the wrong place.
 *
 * This rewrites single-backtick code spans containing `\`` into double-backtick
 * delimited spans with the backslash removed:
 *   Input:  `foo(\` bar)`
 *   Output: `` foo(` bar) ``
 */
export function fixMarkdownBackticks(text: string): string {
  // Match a single backtick (not preceded by another backtick), then content
  // that includes at least one \`, then a closing backtick (not followed by
  // another backtick). We need to be careful not to touch already-valid
  // double-backtick spans or fenced code blocks.
  //
  // Strategy: walk through the string looking for single-backtick code spans
  // that contain escaped backticks. We avoid fenced code blocks by only
  // matching within single lines (no newlines in the span).

  return text.replace(
    /(?<!`)(`)((?:[^`\\\n]|\\.)*\\`(?:[^`\\\n]|\\.)*)`(?!`)/g,
    (_match, _open: string, inner: string) => {
      // Remove backslash escapes before backticks
      const fixed = inner.replace(/\\`/g, "`");
      return `\`\` ${fixed} \`\``;
    },
  );
}
