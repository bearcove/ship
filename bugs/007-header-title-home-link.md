# 007: Header title does not navigate home

Status: open
Owner: frontend

## Symptom

Clicking the `Ship` title in the header does nothing.

## Expected Behavior

The header title should navigate back to the session list / homepage.

## Evidence

User report from live use.

## Suspected Root Cause

The title is rendered as static text instead of a link or button wired to the session-list route.

## Spec Impact

Navigation affordance and basic usability.

## Next Action

- Make the header title a home link
- Ensure it works consistently from both the session list and session detail views
