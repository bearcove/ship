# 012: Session header should be breadcrumb navigation, not a close bar

Status: open
Owner: frontend

## Symptom

The current session header is not serving navigation well:
- the top title/header is not acting as proper breadcrumb navigation
- there is a separate top-right `x` close affordance for leaving the session
- session context is spread across multiple header elements instead of one clear breadcrumb line

## Expected Behavior

Use the top bar as breadcrumb/navigation:

```text
ship :: [project] :: [branch]     (filler) [mute icon]
```

This would allow:
- clicking `ship` to navigate to the session list
- clicking the project crumb to navigate to that project's filtered session list
- branch/session context to stay visible in one place
- removal of the separate `x` close affordance, since normal navigation handles leaving the session view

## Evidence

User design feedback from live use of the session view.

## Suspected Root Cause

The current session header is mixing context display and escape/navigation in a way that forces a separate close control instead of making the header itself navigable.

## Spec Impact

Session-view navigation chrome and header interaction model.

## Next Action

- redesign the session header as breadcrumb navigation
- wire header segments to the correct routes
- remove the redundant session close/leave `x` if breadcrumb navigation covers that path cleanly
- evaluate whether mute belongs as the sole trailing control in that bar
