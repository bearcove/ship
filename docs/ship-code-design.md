# ship-code: Structural code operations with shadow commits

## Overview

A new crate (`ship-code`) providing code search, editing, and snapshot
management for agents. Exposed through the existing ship-server MCP as a
single multi-operation tool. All mutations are backed by shadow git commits
for unlimited undo.

## Architecture

```
ship-server (MCP layer)
  └── ship-code (engine)
        ├── search (rg + future tree-sitter symbol index)
        ├── edit (line-range, find/replace, future structural)
        └── snapshots (shadow git commits)
```

`ship-code` is a library crate. It knows nothing about MCP. It takes JSON
operation objects and returns JSON results. `ship-server` imports it and
wires it up as MCP tool(s).

## Shadow commits

Every mutation (edit, command execution) creates an automatic git commit on
a shadow branch. These are invisible to the agent.

- **Automatic**: after every edit or command that modifies files
- **Undo**: `undo(snapshot: N)` resets to that shadow commit
- **Real commits**: when the agent calls `commit("message")`, all shadow
  commits since the last real commit are squashed into one clean commit
  with the agent's message
- **Submission**: captain sees only the clean commit history

Implementation: `git reset --soft <last-real-commit> && git commit -m "msg"`

Shadow commits reset on each real commit. If an agent accumulates 100+
shadow commits without committing, nudge them.

## MCP tool interface

One tool: `code`. Takes a JSON object (or array of objects for batching).
Returns results for each operation including diffs where applicable.

## Operations

### search

Find text and symbols in the codebase.

```json
{
  "op": "search",
  "query": "notify_captain",
  "path": "crates/ship-server/src",  // optional, scope search
  "case_sensitive": false,            // optional, default false
  "file_glob": "*.rs"                // optional
}
```

Response has two sections:

- **Symbol matches** (future, tree-sitter index): function/struct/impl with
  name, file, line range, parent scope. Body included if under 50 lines,
  otherwise just signature + range.
- **Text matches**: file, line number, matching line, context.

Regex handling: accept whatever the agent sends. Try as literal first, then
ERE, then BRE. Never error on dialect mismatch.

### edit

Modify files. Returns unified diff and snapshot number.

```json
{
  "op": "edit",
  "file": "src/ship_impl.rs",
  "edits": [
    {
      "type": "replace_lines",
      "start": 145,
      "end": 160,
      "content": "fn new_version() {\n    // ...\n}"
    }
  ]
}
```

Edit types:

- **replace_lines**: replace line range with new content
- **insert_lines**: insert content at line number
- **delete_lines**: delete line range
- **find_replace**: text find/replace (with optional `replace_all`)
- **replace_node**: (future) replace a tree-sitter node by query
- **move_node**: (future) move a node to another file
- **delete_node**: (future) delete a node by query

Response:

```json
{
  "snapshot": 7,
  "diff": "--- a/src/ship_impl.rs\n+++ b/src/ship_impl.rs\n@@ ...",
  "shadow_count": 7,
  "nudge": null
}
```

When `shadow_count` exceeds 100:

```json
{
  "nudge": "You have 102 uncommitted edits. Consider committing."
}
```

### undo

Restore worktree to a previous snapshot.

```json
{
  "op": "undo",
  "snapshot": 5
}
```

Returns diff from current state to restored state.

### read_node (future)

Read a specific syntax node without pulling the whole file.

```json
{
  "op": "read_node",
  "file": "src/ship_impl.rs",
  "query": "fn notify_captain_progress"
}
```

Returns the node body, line range, and parent scope.

## Phased rollout

### Phase 1: Foundation
- `search` with text matches (rg wrapper, regex dialect fixing)
- `edit` with line-range operations (replace, insert, delete, find/replace)
- Shadow commit snapshots + undo
- Auto-snapshot after command execution

### Phase 2: Tree-sitter
- Symbol index extracted from tree-sitter parse (via arborium)
- Symbol matches in search results
- `read_node` operation
- Structural edit operations (replace_node, move_node, delete_node)

### Phase 3: Telemetry
- Log all tool calls (operation type, query, success/failure)
- Analyze real search patterns to refine the interface

## Crate structure

```
crates/ship-code/
  src/
    lib.rs          -- public API: execute(op) -> result
    search.rs       -- rg wrapper, regex fixing
    edit.rs         -- line-range edits
    snapshot.rs     -- shadow commit management
    ops.rs          -- operation JSON types (serde)
```
