# 040: "Mate needs permission" banner pushes down the entire layout

Status: open
Owner: frontend

## Symptom

~3:05 — When the mate needs a permission approval, a banner appears at the very top of the session view: "Mate needs permission approval before it can continue." This shifts the entire layout down and is visually jarring.

## Expected behavior

Permission notifications should be inline in the mate's panel, not a full-width top banner. The mate's feed already shows the permission block — the banner is redundant and disruptive.

## Next action

- Remove the top banner for mate permission state
- The permission block in the mate feed is sufficient — scroll to it or highlight it
- If an indicator is needed outside the feed, use a subtle badge on the Mate panel header
