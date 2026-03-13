# Agent Structural Navigation for Ship

## Executive Summary

Ship currently gives agents `read_file` plus shell-oriented search (`rg`, `fd`, `run_command`). That is enough to eventually find code, but it does not reliably produce good investigation behavior. Agents often burn turns on long chains of blind `read_file` calls, re-discover context the captain already supplied, and misuse `rg` as if it were basic grep.

The recommended v1 direction is a lightweight structural navigation layer, built in-process with tree-sitter or an equivalent parser/index approach, for Rust, TypeScript/TSX, and Markdown. The layer should expose three agent-facing tools:

- `file_outline` for fast orientation inside one file
- `search_symbols` for workspace-wide symbol lookup
- `read_symbol` for targeted reads of one declaration or Markdown section

This should ship together with explicit behavior-shaping mechanisms:

- a preferred search ladder in the mate prompt and tool descriptions
- runtime feedback when agents overuse blind `read_file`
- targeted guardrails for common bad search patterns, especially incorrect `rg` escaping

This is intentionally not an LSP design. Ship does not need background language servers, semantic rename, or type-aware refactors to solve the current problem.

## Problem

### Current pain points

- Ship has no structural navigation layer for file outlines or workspace symbol lookup.
- Agents often use `read_file` as the first and only discovery primitive, even for large Rust or TSX files.
- When the captain already supplied files and a plan, the mate still frequently spends many turns re-exploring the same area.
- The current shell guidance helps somewhat, but agents still misuse text search tools. A recurring example is escaping `|` in `rg` as if ripgrep used BRE syntax.
- Raw text search answers different questions than structural navigation, but today Ship does not enforce that distinction.

### Why this matters

The main issue is not only missing capability. It is missing discipline. Even with a better parser-backed tool, Ship will keep wasting turns unless the runtime, prompts, and tool descriptions make the intended search order concrete and enforceable.

## Goals

- Reduce exploratory `read_file` churn before implementation begins.
- Give agents a fast path for “what symbols are in this file?” and “where is this symbol defined?”
- Make symbol-targeted reads smaller and more actionable than whole-file reads.
- Preserve raw `rg` and `fd` for the jobs they are actually good at.
- Keep the design practical for Ship’s current Rust backend, roam RPC layer, and MCP tool model.
- Support Rust, TypeScript/TSX, and Markdown in v1.

## Non-Goals

- Full LSP process management.
- Cross-file semantic analysis such as type inference, references, rename, or “go to implementation”.
- Human-facing IDE features.
- Structural editing or refactoring in v1.
- Perfect parsing of broken files before any implementation can proceed.

## Recommended Tool Surface

### Recommendation

Expose three separate tools, not one overloaded navigation tool:

1. `file_outline(path)`
2. `search_symbols(query, filters?)`
3. `read_symbol(symbol_ref, options?)`

A smaller single-tool surface was considered, but separate tools are the better fit for agent behavior. The tool choice becomes obvious from the question the agent is asking, the result shapes stay predictable, and Ship can write much sharper tool descriptions and guardrails.

### `file_outline`

Use when the agent already knows the file path and needs fast structure before reading bodies.

Suggested response shape:

- `path`
- `language`
- `status`: `ok`, `partial`, `unsupported_language`, or `parse_error`
- `symbols`: ordered by source position
- `truncated`: boolean when the symbol list exceeds the response cap
- `fallback_hint`: present only for unsupported or failed parses

Each symbol entry should include:

- `symbol_id`: opaque identifier valid for the current indexed snapshot
- `kind`: for example `module`, `struct`, `enum`, `trait`, `impl`, `fn`, `method`, `const`, `type`, `component`, `heading`
- `name`
- `signature`: short display form, omitted when not meaningful
- `line_start`
- `line_end`
- `parent_symbol_id`: optional
- `container_name`: optional short human-readable parent context
- `child_count`

Markdown should be treated structurally in v1. Headings are navigable symbols, and `read_symbol` for Markdown should read a section body until the next heading of the same or higher level.

### `search_symbols`

Use when the agent knows a symbol name or partial identifier but not the file.

Suggested response shape:

- `query`
- `status`: `ok`, `partial`, `no_results`, `unsupported_scope`
- `results`: ranked matches
- `truncated`: boolean when matches exceed the response cap
- `indexed_languages`: list of languages searched
- `fallback_hint`: present when the result set is partial or outside indexed scope

Each result should include:

- `symbol_id`
- `kind`
- `name`
- `signature`
- `path`
- `line_start`
- `line_end`
- `container_name`
- `rank`
- `match_reason`: `exact`, `prefix`, `substring`, or `fuzzy`

Ranking should be simple and deterministic:

- exact name match first
- prefix match second
- substring token match third
- fuzzy fallback last
- ties broken by path filter match, then shorter path, then earlier source position

### `read_symbol`

Use after `file_outline` or `search_symbols` when the agent wants the body of one declaration or Markdown section instead of paging through the entire file.

Suggested response shape:

- `path`
- `language`
- `status`: `ok`, `symbol_not_found`, `unsupported_language`, or `parse_error`
- `symbol`: metadata for the resolved symbol
- `excerpt_start_line`
- `excerpt_end_line`
- `numbered_excerpt`
- `truncated`
- `fallback_hint`: present on failures

`read_symbol` should accept either:

- a `symbol_id` returned by a previous structural tool call, or
- a `(path, name)` pair as a convenience fallback

The primary read should be the exact symbol span, not an arbitrary fixed-size snippet. Optional surrounding context can be added later, but v1 should optimize for precision.

## Why Three Tools Instead of One

A single `navigate_code` tool would look smaller on paper, but it makes the hard part worse:

- the prompt has to explain multiple modes and decision branches inside one tool
- results become polymorphic and harder for the agent to use correctly
- runtime feedback becomes less precise because Ship cannot tell whether the agent is orienting, searching, or reading

Three tools are a better fit for Ship’s current MCP style, where tool descriptions do a meaningful amount of steering.

## Structural Index Design

### Index architecture

Use an in-process per-worktree index owned by the Ship backend.

Recommended properties:

- Parser-backed outlines and symbol tables for Rust, TypeScript/TSX, and Markdown.
- No separate LSP server processes.
- Lazy build and lazy refresh on first structural query.
- Per-file invalidation keyed by path plus file metadata such as mtime and size, with content hashing allowed as an implementation refinement.
- Shared index data reusable by both captain and mate sessions for the same worktree.

### Freshness expectations

The index must not knowingly return stale symbol spans.

Recommended freshness model:

- Before answering a structural query, Ship checks whether the relevant file entries are stale.
- Stale files are reparsed synchronously for that request.
- Workspace symbol search may lazily refresh only the changed files rather than rebuilding the full index.
- Optional background warm-up is fine, but not required for correctness.

### Parse failure and unsupported languages

For unsupported languages, Ship should not fabricate structure. It should return a structured failure plus a fallback hint to use `read_file`, `search_files`, or `run_command` with `rg`.

For supported languages with recoverable parse errors, Ship should return best-effort results marked `partial` when it can still extract top-level symbols. If no usable symbol information can be recovered, return `parse_error` with a fallback hint.

## Raw Search After Structural Navigation Lands

Raw text search remains important, but its role becomes narrower and explicit.

### Structural tools should be the default for

- finding a function, method, component, struct, enum, trait, module, type alias, or Markdown section
- understanding file shape before reading source bodies
- jumping directly to the declaration the agent intends to inspect

### `rg` or `search_files` should remain the default for

- string literals, log text, error messages, comments, TODOs, and prose fragments
- exact syntax fragments rather than named declarations
- unsupported languages or parse failures
- checking whether a literal appears across generated or non-indexed files
- cases where the agent does not know whether the target is a symbol at all

### `fd` or `list_files` should remain the default for

- locating files by path or filename pattern
- narrowing a directory subtree before structural lookup

### `read_file` should remain justified for

- small config or manifest files where an outline adds no value
- unsupported languages
- wide context after a targeted `read_symbol`
- captain-supplied exact line ranges

## Behavior Model: Investigation Discipline

Ship should encode an explicit search ladder for agents.

### Preferred search ladder

1. Start with captain-supplied files and plan if present.
2. If the target is a known supported source file, call `file_outline` before `read_file` unless the file is already known to be small or the exact line range is already known.
3. If the target is a named declaration but the file is unknown, call `search_symbols`.
4. After structural discovery, use `read_symbol` for the exact declaration or section.
5. Use `read_file` only when targeted structural reads are insufficient.
6. Use `rg` or `search_files` for non-symbol text lookup.
7. Use `fd` or `list_files` for path discovery only.

### How pre-supplied context changes behavior

When the captain already supplied files or a step-by-step plan, Ship should treat broad rediscovery as suspicious by default.

Expected behavior:

- The mate should begin in the supplied area unless it quickly finds contradictory evidence.
- Structural lookup outside the supplied area is still allowed, but it should be exceptional rather than the default opening move.
- Blind `read_file` exploration should trigger guidance sooner when a plan or file context already exists.

## Prompting and Tool Description Changes

### Mate prompt changes

The mate system prompt should explicitly name the structural search ladder. It should not only say “read the relevant files.” That wording currently leaves too much room for blind exploration.

Recommended additions:

- “For Rust, TypeScript/TSX, and Markdown, use `file_outline`, `search_symbols`, and `read_symbol` before broad `read_file` exploration.”
- “If the captain supplied files or a plan, start there. Do not re-map the repository unless the supplied context is insufficient.”
- “Use `rg` for literals and text patterns, not for symbol discovery.”

### Tool description changes

Update tool descriptions so the first-use path is obvious.

- `read_file`: say that for supported code files, `file_outline` or `read_symbol` is usually the better first step.
- `run_command`: keep the current `rg` and `fd` guidance, but also say that raw shell search is secondary to structural tools for symbol lookup.
- `search_files`: include explicit ripgrep examples that demonstrate alternation without BRE-style escaping.

## Guardrails and Feedback Loops

### Bad `rg` usage

Ship should add a targeted runtime guardrail for common ripgrep misuse, not just static prose in the tool description.

Recommended v1 rule:

- If the agent invokes `rg` with a pattern containing `\|`, reject the command before execution and return a corrective message explaining that ripgrep uses Rust regex syntax and alternation should be written as `foo|bar`, not `foo\|bar`.

This is narrow by design. It directly addresses a recurring failure mode without trying to build a full shell linter.

### Blind read detection

Ship should track exploratory read behavior per task and inject one-shot guidance when the agent is clearly thrashing.

Recommended v1 counters:

- number of `read_file` calls on supported-language source files
- number of those reads that were not preceded by `file_outline`, `search_symbols`, or `read_symbol`
- number of repeated paged reads against the same file without structural lookup
- whether captain-supplied file context or a pre-supplied plan existed

Recommended v1 thresholds:

- If captain-supplied context exists, inject guidance after 4 blind reads of supported-language files.
- Otherwise, inject guidance after 8 blind reads of supported-language files.
- Independently, if the same supported file is paged 3 times without a prior `file_outline` or `read_symbol`, inject targeted guidance for that file.

The guidance should be specific, not generic. Example shape:

- mention the file or symbol area involved
- name the structural tools to use next
- explain why Ship believes the current exploration pattern is wasteful

### Scope of guardrails

These should be soft guardrails, not hard bans. Ship should steer the agent toward a better search path, not make legitimate debugging impossible.

The only hard search-specific rejection proposed for v1 is the `rg` alternation misuse rule because the correction is precise and low-risk.

## What Belongs in Spec vs Design

The spec should cover externally visible behavior and any runtime rules that are meant to be relied on or tested:

- the new tool names and arguments
- response shape requirements that matter to agents
- supported-language and fallback behavior
- freshness guarantees at the behavioral level
- mate prompt and tool-description obligations
- concrete guardrail triggers and corrective behavior

The design doc should carry the implementation choices that may reasonably evolve:

- exact parser library choice
- internal cache layout
- exact symbol extraction logic per language
- telemetry details
- whether to warm the index in the background

## Alternatives Considered

### Full LSP integration

Rejected for v1. LSP would add process lifecycle management, language-server availability problems, higher memory cost, and more surface area than Ship needs for basic agent navigation.

### Prompt-only fix

Rejected. Better prompt text alone will not solve repeated blind reads or recurring `rg` misuse.

### Raw shell tooling only

Rejected. `rg` and `fd` are useful, but they are not good substitutes for file outlines or symbol-targeted reads.

### Single structural tool

Rejected. It is worse for teaching, worse for telemetry, and worse for targeted runtime guidance.

## Rollout Order

1. Add spec requirements for structural navigation, fallback behavior, prompting, and guardrails.
2. Update agent-facing prompt text and MCP tool descriptions to reflect the new search ladder.
3. Add the typed roam and MCP surface for `file_outline`, `search_symbols`, and `read_symbol`.
4. Implement the in-process structural index for Rust, TypeScript/TSX, and Markdown with lazy freshness checks.
5. Add runtime search guardrails and read-thrash detection.
6. Add tests that verify tool results, fallback behavior, freshness, and guardrail triggering.

## Open Questions and Deferred Choices

- Should captains get the same structural navigation tools immediately, or should v1 land mate-first and then widen to captain?
- Should Markdown expose heading level as part of `kind` or as a separate numeric field?
- How stable does `symbol_id` need to be across reparses within one task?
- Should best-effort partial parse results be on by default for malformed files, or should Ship fail closed more often?
- Should generated files be indexed in v1, or indexed but ranked lower than handwritten source?