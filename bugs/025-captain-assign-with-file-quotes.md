# Captain should quote relevant files when assigning a task to the mate

When the captain calls `captain_assign`, it has already explored the codebase and knows which files (and which line ranges) are relevant to the task. Instead of the mate redoing all that exploration from scratch, the captain should be able to include file quotes in the assignment.

The captain specifies: path + start line + end line for each relevant file. Ship reads those ranges and injects them directly into the mate's context as the first message, before the task description. This way the mate starts with full context and can go straight to implementation.

This likely means:
- A new field on `captain_assign`: `file_refs: Vec<{ path: String, start_line: usize, end_line: usize }>`
- Ship reads the file ranges from the worktree at assignment time
- The mate's initial context block includes the quoted snippets before the task prompt
