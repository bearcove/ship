# 041: "Always" permission approval has unclear scope

Status: open
Owner: frontend

## Symptom

~3:20 — When the mate requests permission for e.g. `cargo doc`, there are approval options including "Always". The user doesn't know if "Always" means:
- Always approve `cargo doc` specifically
- Always approve any `cargo` command
- Always approve for this session
- Always approve permanently

There is no indication of scope anywhere in the UI.

## Expected behavior

The scope of "Always" must be explicit. Either:
- Show the exact rule being created ("Always allow: cargo doc")
- Or don't offer "Always" until the policy system (bug 037) is implemented

## Next action

- Add scope description to the permission dialog when "Always" is selected
- Or remove "Always" option pending proper policy implementation
