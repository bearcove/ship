# 037: Mate should not have raw filesystem/shell ACP capabilities

Status: open
Owner: backend

## Symptom

The mate currently gets standard ACP capabilities including direct filesystem and terminal access. This means every file read/write/shell command triggers a permission prompt, and the boundary between "what Ship controls" and "what the agent does raw" is blurry.

## Expected behavior

The mate gets no raw filesystem or shell capabilities from ACP. All file and command access goes through MCP tools that Ship exposes — read file, write file, run tests, etc. These are higher-level, already-scoped, and require no per-action permission prompts because the tools themselves define the boundary.

Result: zero permission prompts during normal mate operation.

## Next action

- Strip filesystem/terminal ACP capabilities from mate agent config
- Ensure all necessary operations are covered by MCP tools Ship exposes to the mate
- Update `mate.capabilities` spec rule
