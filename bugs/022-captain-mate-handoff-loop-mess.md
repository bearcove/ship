# Captain/mate handoff after task completion is a mess

Multiple problems in the sequence of events when mate finishes work and captain reviews:

## Task notification is confusing
- When mate updates are injected into captain's thread, we re-include the full Task text (not just a title)
- The prompt doesn't lead with "YOUR MATE HAS AN UPDATE" — it's confusing even for humans
- "Commit: skipped (worktree clean)" message should be suppressed when there's nothing to report

## Plan update spam
- Mate spams plan_step_complete calls (because they have to, to submit — see bug 017)
- These are not debounced at all
- Captain starts reviewing before all updates land, causing confusion

## Captain doesn't recognize mate already submitted
- Captain says "Once the mate reports the commit and verification output, I can do the final acceptance pass" even though mate already submitted
- Then mate submits again, captain starts reviewing
- Then captain gets: "The mate stopped repeatedly without submitting. Here is a reconstructed summary of recent work: No recent output available." — despite mate having submitted TWICE
- Captain then reassigns the same work to the mate again

## Root causes
1. Mate→captain notification format is bad (full task dump, no clear "UPDATE" header)
2. Plan updates not debounced/batched
3. Captain's state tracking of mate submissions is broken — doesn't recognize completed submissions
4. The "stopped without submitting" fallback fires incorrectly
