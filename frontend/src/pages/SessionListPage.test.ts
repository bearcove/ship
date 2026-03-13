import { describe, expect, it } from "vitest";
import { agentKindTooltip } from "./session-list-utils";

// r[verify ui.session-list.create]
// r[verify server.agent-discovery]
describe("agentKindTooltip — create-session dialog states from discovery", () => {
  it("returns undefined when all agents are available", () => {
    const discovery = { claude: true, codex: true, opencode: true };
    expect(agentKindTooltip("claude", discovery)).toBeUndefined();
    expect(agentKindTooltip("codex", discovery)).toBeUndefined();
    expect(agentKindTooltip("opencode", discovery)).toBeUndefined();
  });

  it("shows claude-agent-acp tooltip when claude is unavailable", () => {
    const discovery = { claude: false, codex: true, opencode: true };
    expect(agentKindTooltip("claude", discovery)).toBe("claude-agent-acp not found on PATH");
    expect(agentKindTooltip("codex", discovery)).toBeUndefined();
    expect(agentKindTooltip("opencode", discovery)).toBeUndefined();
  });

  it("shows codex-acp tooltip when codex is unavailable", () => {
    const discovery = { claude: true, codex: false, opencode: true };
    expect(agentKindTooltip("claude", discovery)).toBeUndefined();
    expect(agentKindTooltip("codex", discovery)).toBe("codex-acp not found on PATH");
    expect(agentKindTooltip("opencode", discovery)).toBeUndefined();
  });

  // r[verify acp.binary.opencode]
  it("shows opencode tooltip when opencode is unavailable", () => {
    const discovery = { claude: true, codex: true, opencode: false };
    expect(agentKindTooltip("claude", discovery)).toBeUndefined();
    expect(agentKindTooltip("codex", discovery)).toBeUndefined();
    expect(agentKindTooltip("opencode", discovery)).toBe("opencode not found on PATH");
  });

  it("shows tooltips for all when no agent is available", () => {
    const discovery = { claude: false, codex: false, opencode: false };
    expect(agentKindTooltip("claude", discovery)).toBe("claude-agent-acp not found on PATH");
    expect(agentKindTooltip("codex", discovery)).toBe("codex-acp not found on PATH");
    expect(agentKindTooltip("opencode", discovery)).toBe("opencode not found on PATH");
  });
});
