# 034: No image drag-and-drop or upload in composer

Status: open
Owner: frontend

## Symptom

There is no way to attach an image (e.g. a screenshot) to a steer or prompt. You can only communicate in text.

## Expected behavior

Dragging an image onto the composer pane (or clicking an attach button) attaches it to the next message. The image is sent to the agent as a vision input.

## Next action

- Add drag-and-drop target to InlineAgentComposer
- Add paste-from-clipboard support (Ctrl+V with image in clipboard)
- Backend needs to pass image content through ACP prompt
