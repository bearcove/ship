# 045: After accepting mate work, the session state is unclear

Status: open
Owner: frontend

## Symptom

~8:16 — After clicking "Accept mate work", the task was accepted and both captain and mate went idle. The user said "I don't know what the state of anything is." There was no indication of what was committed, what the next step is, or how to continue.

## Expected behavior

After accept:
- Show a brief summary of what was completed (task description + status = Accepted)
- Ideally show the git diff / commit that was made
- Captain should prompt the user: "Task complete. What's next?"
- The task bar should reflect the completed state clearly

## Next action

- After task acceptance, captain should send a completion acknowledgment to the user
- Show accepted task in a "recent" section or history
- Consider auto-committing mate work on accept (see also bug 042 note about automatic commits)
