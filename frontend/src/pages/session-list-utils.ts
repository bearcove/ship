// r[ui.session-list.create]
// r[server.agent-discovery]
export function agentKindTooltip(
  kind: "claude" | "codex" | "opencode",
  discovery: { claude: boolean; codex: boolean; opencode: boolean },
): string | undefined {
  if (kind === "claude" && !discovery.claude) return "claude-agent-acp not found on PATH";
  if (kind === "codex" && !discovery.codex) return "codex-acp not found on PATH";
  if (kind === "opencode" && !discovery.opencode) return "opencode not found on PATH";
  return undefined;
}
