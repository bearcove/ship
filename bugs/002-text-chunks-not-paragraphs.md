# 002: Text chunks render as separate block elements

Status: open
Owner: frontend

## Symptom

Agent prose is displayed one chunk per block-level element, so normal sentences appear vertically fragmented instead of as paragraphs.

## Expected Behavior

Consecutive text chunks should accumulate and render as normal paragraph text.

## Evidence

Observed in the session UI: each segment is its own block/div, producing output like one word or short phrase per line.

## Suspected Root Cause

The frontend is likely rendering chunk boundaries too literally instead of treating accumulated text blocks as prose.

## Spec Impact

Affects readability of agent text and likely violates the intended text block rendering behavior.

## Next Action

- Inspect text block accumulation and rendering in the frontend event/block store path.
- Confirm whether the backend is already coalescing consecutive text chunks correctly.
- Add a UI regression test for multi-chunk prose rendering.
