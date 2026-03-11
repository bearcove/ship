# captain_accept should land work on main via rebase + fast-forward merge

Currently `captain_accept` marks the task done but leaves the commits stranded on the session branch. The full intended flow is:

1. Mate finishes work, captain reviews it
2. Captain calls `captain_notify_human` with the full `git diff main...HEAD` and the worktree path — UI shows the diff, "Open in VS Code" / "Open Terminal" buttons, and Approve / Request Changes actions
3. Human inspects (optionally pokes around in the worktree manually), then approves
4. Captain calls `captain_accept`
5. Ship runs `git rebase main` inside the session worktree
6. Ship runs `git merge --ff-only ship/{session-id}/session` in the main repo — clean linear history, no merge commits
7. Worktree is now clean and ready for the next task

**Conflict handling:** if the rebase hits conflicts, ship surfaces them back to the captain (new session state). The captain can delegate to the mate to resolve conflicts and `git rebase --continue`, or escalate to the human.

**Frontend work needed:**
- Human review panel: render diff, show worktree path with editor/terminal buttons, Approve / Request Changes buttons
- Enrich `captain_notify_human` payload with `worktree_path` and `diff` fields so the UI can render them properly
