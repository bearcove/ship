# 011: Steering input UX is modal and asymmetric

Status: open
Owner: frontend

## Symptom

The current steering UX is awkward:
- `Steer the Mate directly` uses a separate dialog instead of an inline chat-style input
- `Cmd+Return` does not submit the steer form
- there is no equivalent direct steer input for the captain

## Expected Behavior

- steering should behave like a normal chat input at the bottom of the relevant agent feed
- `Cmd+Return` should submit the steer message
- both mate and captain should have symmetric steer inputs
- the session view should use two regular chat inputs at the bottom, one per agent feed, rather than a special modal flow for only one role

## Evidence

User report from live use of the session view and steer flow.

## Suspected Root Cause

The current design treats steering as a special-case modal action instead of part of the normal conversation surface, and only exposes that path for the mate.

## Spec Impact

Session interaction UX, steer/review flow ergonomics, and keyboard affordances.

## Next Action

- evaluate whether the current dialog-based steer flow should be replaced by inline per-agent chat inputs
- add `Cmd+Return` submission behavior
- design a symmetric captain + mate direct steer surface
