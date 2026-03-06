# 015: Terminal block can show raw ACP debug output instead of terminal content

Status: open
Owner: fullstack

## Symptom

Terminal/tool-call blocks can render raw Rust/ACP debug dumps such as:

```text
[
  Content(
    Content {
      content: Text(
        TextContent {
          ...
```

instead of a clean terminal/result view.

## Expected Behavior

Terminal blocks should show the actual command/result content in a readable terminal-style presentation, not a debug serialization of internal ACP content structures.

## Evidence

User screenshot showing a terminal block rendering a raw debug dump with escaped ANSI sequences and ACP content wrappers.

## Suspected Root Cause

Some ACP content is still being stringified with debug formatting (`{:#?}` or equivalent) instead of being mapped to a user-facing terminal/result representation.

## Spec Impact

Tool-call/terminal block rendering quality and readability.

## Next Action

- trace where ACP tool/terminal content is converted into Ship block/result text
- remove remaining debug-serialization paths from user-visible output
- add a rendering regression test for terminal/result content
