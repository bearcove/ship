# 051: Strip raw ACP capabilities from mate and captain

Owner: backend

## What

Once bugs 046–050 are implemented (Ship-controlled MCP tools for the mate):

- **Mate**: Remove raw ACP filesystem and shell capabilities. The mate operates exclusively through Ship MCP tools. Zero permission prompts during normal operation.
- **Captain**: Remove ACP explore/subagent capabilities. The captain only has Ship MCP tools (captain_assign, captain_steer, captain_accept, captain_cancel, captain_notify_human). If it needs codebase context, that's what the mate is for.

## Depends on

- 046 (read_file)
- 047 (write_file)
- 048 (edit_prepare/confirm)
- 049 (search/list files)
- 050 (run_command)

## Next action

- Audit ACP config for both agents
- Strip filesystem/terminal/explore capabilities
- Verify the mate can still do all necessary work through Ship MCP tools
- Subsumes bugs 037 and 038
