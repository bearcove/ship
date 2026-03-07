# 033: No file mention support in composer

Status: open
Owner: frontend

## Symptom

When steering the mate or prompting the captain, there is no way to reference a file by path. You have to describe it in prose.

## Expected behavior

Typing `@` in the composer should open a file picker / autocomplete that inserts a file mention. The backend includes the file contents (or a reference) in the prompt sent to the agent.

## Next action

- Implement `@`-triggered file autocomplete in InlineAgentComposer
- Backend needs to resolve file mentions and inject content into the prompt
