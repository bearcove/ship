import type { SessionSummary } from "../generated/ship";

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

export function sortSessions(sessions: SessionSummary[]): SessionSummary[] {
  const priority = (session: SessionSummary) => {
    if (session.is_admiral) return -1;
    const tag = session.task_status?.tag;
    if (tag === "ReviewPending" || tag === "SteerPending") return 0;
    if (tag === "Working" || tag === "Assigned") return 1;
    return 2;
  };

  return [...sessions].sort((a, b) => priority(a) - priority(b));
}
