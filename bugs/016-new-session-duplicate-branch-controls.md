# 016: New session branch chooser is split into two controls

Status: open
Owner: frontend

## Symptom

The new-session dialog now has both:
- a freeform branch text input
- a separate branch dropdown/trigger

This makes the base-branch selection UI feel duplicated and confusing.

## Expected Behavior

The base branch should be chosen through one coherent searchable combobox, not two separate controls that appear to do the same job.

## Evidence

User report from live use, with a screenshot showing both a text input and a dropdown for base branch in the same dialog.

## Suspected Root Cause

The searchable branch-picker implementation was added by layering a text input on top of an existing trigger-based selection UI instead of collapsing them into one control.

## Spec Impact

New-session branch selection UX.

## Next Action

- replace the duplicated branch controls with one searchable combobox
- make the visible value and selected branch stay in sync
- add interaction coverage for branch selection and submission
