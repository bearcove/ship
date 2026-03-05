# Ship Frontend Specification

Frontend-specific requirements: technology stack, styling, component library,
routing, testing, packaging, and UI design.

## Frontend Stack

r[frontend.typescript]
The frontend MUST be implemented in TypeScript with strict mode enabled.

r[frontend.react]
The frontend MUST use React 19 for UI rendering.

r[frontend.vite]
The frontend MUST use Vite 6 as its dev server and build tool, with the
`@vitejs/plugin-react` plugin.

r[frontend.codegen]
Frontend types MUST be generated from the backend's Rust traits via
roam-codegen.

### Styling

r[frontend.style.vanilla-extract]
The frontend MUST use vanilla-extract for styling. All styles MUST be defined
in `.css.ts` files as typed TypeScript, producing zero-runtime static CSS at
build time.

r[frontend.style.vite-plugin]
The Vite config MUST include the `@vanilla-extract/vite-plugin` for build-time
CSS extraction.

### Component Library

r[frontend.components.radix]
The frontend MUST use Radix Themes (`@radix-ui/themes`) as its pre-styled
component library for buttons, dialogs, dropdowns, badges, cards, tabs, and
layout primitives.

r[frontend.components.radix-theme]
The Radix Theme provider MUST be configured at the app root with a dark
appearance and accent color suitable for a developer tool.

r[frontend.components.radix-overrides]
Where Radix Themes defaults are insufficient, overrides MUST be applied via
vanilla-extract using Radix's CSS custom properties, not by fighting the
component internals.

### Routing

r[frontend.routing]
The frontend MUST use react-router-dom v7 for client-side routing between the
session list view and individual session views.

### Icons

r[frontend.icons]
The frontend MUST use `@phosphor-icons/react` for iconography.

### Testing

r[frontend.test.vitest]
Frontend tests MUST use vitest as the test runner.

r[frontend.test.rtl]
Frontend component tests MUST use `@testing-library/react` for rendering and
assertions.

### Package Structure

r[frontend.package.private]
The frontend MUST be a private npm package (`"private": true`) in a `frontend/`
directory at the repository root.

r[frontend.package.type-module]
The frontend package MUST use `"type": "module"` for ES module support.

r[frontend.package.scripts]
The frontend package MUST define at minimum these scripts: `dev` (vite dev
server), `build` (typecheck + vite build), `typecheck` (tsc --noEmit), and
`test` (vitest run).

## UI Design

This section specifies how each part of the UI is rendered, which Radix Themes
components are used, and how content blocks and interactions are laid out.

### Layout

r[ui.layout.shell]
The app shell MUST use a full-viewport `Flex` with `direction="column"`. A top
bar contains the app name and global controls. Below it, the main content area
fills the remaining space.

r[ui.layout.session-view]
The session view MUST use a `Grid` with two equal columns for the captain and
mate panels. On viewports narrower than 1024px, the grid MUST collapse to a
single column with a `Tabs` switcher (Captain | Mate) above.

```
┌──────────────────────────────────────────────────┐
│  Ship ∙ session-name          [mode] [close]     │  ← top bar
├────────────────────────┬─────────────────────────┤
│  Captain               │  Mate                   │
│  ┌──────────────────┐  │  ┌───────────────────┐  │
│  │ state + context  │  │  │ state + context   │  │  ← agent header
│  ├──────────────────┤  │  ├───────────────────┤  │
│  │                  │  │  │                   │  │
│  │  event stream    │  │  │  event stream     │  │  ← scrollable
│  │                  │  │  │                   │  │
│  │                  │  │  │                   │  │
│  └──────────────────┘  │  └───────────────────┘  │
├────────────────────────┴─────────────────────────┤
│  Task: implement auth module     [steer] [accept]│  ← task bar
└──────────────────────────────────────────────────┘
```

### Session List

r[ui.session-list.layout]
The session list MUST display sessions as a vertical stack of `Card` components,
each showing: project name (as `Badge`), session branch name, captain and mate
agent kinds (as `Badge`), current task description (truncated), task status
(as `Badge` with color), and a relative timestamp of last activity.

r[ui.session-list.project-filter]
The session list MUST include a project filter above the session cards. The
filter shows "All projects" by default and can be set to a specific project
via a `Select` dropdown. The filter state MUST be preserved in the URL query
string (e.g., `?project=roam`).

r[ui.session-list.empty]
When no sessions exist, the list MUST show a centered `Callout` with
instructions and a "New Session" `Button`. If no projects are registered,
the callout MUST instead prompt the user to add a project first, with an
"Add Project" `Button`.

r[ui.session-list.create]
The "New Session" action MUST open a `Dialog` containing a form with: project
(`Select` populated from registered projects), captain agent kind
(`SegmentedControl`: Claude | Codex), mate agent kind (`SegmentedControl`),
base branch (`Select` populated from the selected project's git branches),
and initial task description (`TextArea`). If only one project is registered,
it MUST be pre-selected.

r[ui.add-project.dialog]
The "Add Project" action (from the empty state callout or a top-bar button)
MUST open a `Dialog` containing: a `TextField` for the repository path
(absolute path, no tilde expansion — the backend resolves it), and a "Add"
`Button`. On submit, the dialog calls `proto.add-project`. If validation
fails (path doesn't exist, not a git repo, name collision), the error MUST
be displayed inline in the dialog as a red `Callout` without closing the
dialog.

r[ui.session-list.create.branch-filter]
The base branch `Select` MUST support type-to-filter for repositories with
many branches. If Radix `Select` proves insufficient, a `TextField` with
a filtered dropdown (combobox pattern) MUST be used instead.

r[ui.session-list.nav]
Each session `Card` MUST be a clickable link (using react-router `Link`)
that navigates to `/sessions/{session_id}`. The entire card surface MUST be
the click target, not a separate "View" button.

r[ui.session-list.status-colors]
Task status badges MUST use these Radix color scales: `Assigned` → gray,
`Working` → blue, `ReviewPending` → amber, `SteerPending` → orange,
`Accepted` → green, `Cancelled` → red.

### Agent Header

r[ui.agent-header.layout]
Each agent panel MUST start with a header row containing: a `Badge` showing
the agent kind (Claude or Codex), a state indicator, and a context usage bar.

r[ui.agent-header.state-indicator]
The agent state MUST be shown as a `Badge` with color coding: `Working` → blue
with a `Spinner` inline, `Idle` → gray, `AwaitingPermission` → amber,
`ContextExhausted` → red, `Error` → red with error icon.

r[ui.agent-header.context-bar]
Context remaining MUST be rendered as a `Progress` component. When below 20%,
the bar MUST switch to the red color scale and a `Callout` with variant "warning"
MUST appear below the header.

### Event Stream

r[ui.event-stream.layout]
Each agent panel MUST contain a `ScrollArea` displaying content blocks in
chronological order. The scroll area MUST auto-scroll to the bottom when new
content arrives, unless the user has scrolled up (sticky-scroll behavior).

r[ui.event-stream.grouping]
Adjacent text blocks from the same prompt turn are already merged into a single
block by the backend (per `event.block-id.text`). Tool calls are single blocks
with a lifecycle status (per `event.content-block.types`) — no client-side
grouping is needed. The frontend renders each block as-is from the store.

### Content Block: Text

r[ui.block.text]
Text content blocks MUST be rendered as markdown using a markdown renderer.
Inline code MUST use the Radix `Code` component. Block-level code fences MUST
be rendered with syntax highlighting (via a lightweight highlighter like
`shiki`). The surrounding container is a plain `Box` with body text styling.

### Content Block: Tool Call

r[ui.block.tool-call.layout]
A tool call block MUST be rendered as a single collapsible unit that updates
in place as patches arrive. While status is `pending` or `running`, the badge
shows a spinner. On `success` or `failure`, the badge updates to a checkmark
or X. The collapsed state shows one line: an icon, the tool name in a `Code`
span, and a status `Badge`. Clicking expands to show arguments and result.

```
▸ Read  src/auth.rs                              ✓
▾ Edit  src/auth.rs                              ✓
  ┌─────────────────────────────────────────────┐
  │ --- a/src/auth.rs                           │
  │ +++ b/src/auth.rs                           │
  │ @@ -10,3 +10,5 @@                           │
  │   fn validate() {                           │
  │ +     check_token();                        │
  │   }                                         │
  │                                             │
  └─────────────────────────────────────────────┘
```

r[ui.block.tool-call.collapsed-default]
Tool calls MUST be collapsed by default. File read tool calls MUST show the
file path in the collapsed line. File write/edit tool calls MUST show the file
path and a diff summary (e.g., "+3 -1").

r[ui.block.tool-call.diff]
File write and edit tool call results MUST render as unified diffs with
additions highlighted in green and deletions in red. The diff MUST use a
monospace `Code` block.

r[ui.block.tool-call.terminal]
Terminal tool calls (command execution) MUST show the command in the collapsed
line. The expanded view MUST show stdout/stderr in a monospace `Code` block
with a maximum height of 20rem and its own `ScrollArea`. Non-zero exit codes
MUST be shown as a red `Badge`.

r[ui.block.tool-call.search]
Search/grep tool calls MUST show the query in the collapsed line and match
results as a list of file:line snippets in the expanded view.

### Content Block: Plan

r[ui.block.plan.layout]
Plan blocks MUST be rendered as an ordered list within a `Card`. Each step
shows its description, priority, and a status icon: `Pending` (circle
outline), `InProgress` (`Spinner`), `Completed` (check icon, green), `Failed`
(X icon, red).

r[ui.block.plan.position]
Plan updates MUST replace the previous plan in the stream, not append. Only
the latest plan is visible. It MUST be rendered as a sticky element at the
top of the agent panel's scroll area (below the header), not inline with
other content blocks.

r[ui.block.plan.filtering]
The frontend MUST filter `Plan` blocks out of the chronological event stream.
They arrive as `BlockAppend`/`BlockPatch` events (per
`event.content-block.types`) but render in the sticky plan area, not in the
scroll feed. All other content block types render in the chronological stream.

### Content Block: Error

r[ui.block.error]
Error content blocks MUST be rendered as a `Callout` with a red color scale,
an error icon, and the error message as body text. If the agent is in the
`Error` state, a "Retry" `Button` MUST appear inside the callout.

### Permission Request

r[ui.permission.layout]
Permission requests MUST be rendered inline in the event stream at the point
where the agent paused. They MUST use a `Card` with an amber border/background,
containing: the tool name in a `Code` span, a human-readable description of
the action, and the arguments in a collapsible detail.

r[ui.permission.actions]
The permission card MUST contain three `Button` components: "Approve" (solid,
green), "Deny" (soft, red), and "Approve all [tool name]" (outline, green).
The "Approve all" button includes a `Tooltip` explaining it applies for the
remainder of the current task.

r[ui.permission.resolved]
After resolution, the permission card MUST update in-place: approved requests
show a green check `Badge`, denied requests show a red X `Badge`. The action
buttons MUST be removed.

r[ui.permission.viewer-mode]
In viewer mode (non-controlling browser), permission action buttons MUST be
disabled with a `Tooltip` explaining "Another browser controls this session."

### Steer Review (Human-in-the-Loop)

r[ui.steer-review.layout]
When the captain produces a steer in human-in-the-loop mode, the steer MUST
appear as a `Card` at the bottom of the session view (above the task bar),
containing: the captain's steer text rendered as markdown, and three action
buttons.

r[ui.steer-review.actions]
The steer review card MUST contain: "Send to Mate" `Button` (solid, blue)
which forwards the steer as-is, "Edit & Send" `Button` (outline, blue) which
opens the steer text in an editable `TextArea` before sending, and "Discard"
`Button` (soft, red) which discards the captain's steer entirely.

r[ui.steer-review.edit-mode]
When "Edit & Send" is clicked, the steer text MUST be replaced by a `TextArea`
pre-filled with the captain's text. A "Send" `Button` and "Cancel" `Button`
appear below. The human can modify the text freely before sending.

r[ui.steer-review.own-steer]
A "Write your own steer" `Button` (outline, gray) MUST always be available in
the task bar, allowing the human to bypass the captain and steer the mate
directly. This opens a `Dialog` with a `TextArea` for the steer message.

### Task Bar

r[ui.task-bar.layout]
The task bar MUST be a horizontal `Flex` pinned to the bottom of the session
view, containing: the current task description (truncated with `Tooltip` for
full text), the task status as a colored `Badge`, and action buttons.

r[ui.task-bar.actions]
Task bar actions depend on task status:
- `Working` → "Cancel" `Button` (soft, red)
- `ReviewPending` → "Accept" `Button` (solid, green), "Cancel" `Button` (soft,
  red), plus the steer review card above if in human-in-the-loop mode
- `SteerPending` → steer review card is the primary action
- `Idle` (no active task) → "New Task" `Button` (solid, blue) which opens a
  `Dialog` with a `TextArea`

r[ui.task-bar.new-task]
The "New Task" button in idle state MUST call `proto.assign` with the task
description from the dialog. This is the same operation used at session
creation — there is no separate "subsequent task" operation.

r[ui.task-bar.history]
A "History" `IconButton` MUST open a `Popover` showing the session's completed
tasks as a `DataList` with task descriptions, statuses, and timestamps.

### Idle Reminders

r[ui.idle.banner]
Idle reminder events MUST be rendered as a pulsing `Callout` with an amber
color scale, appearing at the top of the session view. The callout MUST
describe what is waiting (e.g., "Mate finished — awaiting review" or
"Permission request pending for 2 minutes").

r[ui.idle.badge]
In the session list, sessions with pending idle reminders MUST show a pulsing
amber dot next to the session card.

### Notifications

r[ui.notify.desktop-prompt]
On first visit, the UI MUST display a `Callout` asking the user to enable
desktop notifications, with an "Enable" `Button` that calls
`Notification.requestPermission()`.

r[ui.notify.sound-toggle]
The top bar MUST contain an `IconButton` (speaker icon) that toggles sound
notifications on/off. The current state MUST be persisted in `localStorage`.

### Error States

r[ui.error.agent]
When an agent is in the `Error` state, its entire panel MUST show a `Callout`
with the error message and a "Retry" `Button`. The event stream remains
visible but grayed out below the error callout.

r[ui.error.connection]
If the WebSocket connection drops, a full-width `Callout` with red color scale
MUST appear at the top of the page: "Connection lost — reconnecting..." with
a `Spinner`. On reconnection, it MUST disappear automatically.

### Autonomy Mode Toggle

r[ui.autonomy.toggle]
The session view top bar MUST contain a `Switch` labeled "Autonomous" that
toggles the session's autonomy mode. The current mode MUST also be shown as
a `Badge` (gray for human-in-the-loop, blue for autonomous).

### Theme Configuration

r[ui.theme.config]
The Radix `Theme` provider MUST be configured with: `appearance="dark"`,
`accentColor="iris"`, `grayColor="slate"`, `radius="medium"`,
`scaling="100%"`.

r[ui.theme.dark-only]
Ship is dark mode only. There MUST NOT be a theme switcher or light mode
support. The `appearance` prop is hardcoded to `"dark"`.

r[ui.theme.font]
The app MUST use a monospace font stack for all code-related content (diffs,
terminal output, tool arguments) and the system sans-serif stack for UI text.
Font configuration MUST be applied via vanilla-extract global styles, not
Radix theme overrides.

### Keyboard Shortcuts

r[ui.keys.permission]
When a permission request is focused or the most recent pending request,
pressing `Enter` MUST approve it and `Escape` MUST deny it.

r[ui.keys.steer-send]
In the steer review card and the "write your own steer" dialog, `Cmd+Enter`
(macOS) / `Ctrl+Enter` (other platforms) MUST submit the steer.

r[ui.keys.cancel]
`Escape` MUST close any open `Dialog` or `Popover` (this is Radix default
behavior, listed here for completeness).

r[ui.keys.nav]
`1` and `2` MUST switch focus between the captain and mate panels when no
text input is focused.
