import type React from "react";
import { useEffect, useRef } from "react";
import { useNavigate, useMatch } from "react-router-dom";
import type { SessionSummary } from "../generated/ship";
import type { UnifiedComposerHandle } from "../components/UnifiedComposer";
import { sortSessions } from "../pages/session-list-utils";
import { getShipClient } from "../api/client";

function isEditableTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}

// r[ui.keys.global]
export function useGlobalKeyboard(
  allSessions: SessionSummary[],
  onSessionArchived?: (slug: string) => void,
  composerRef?: React.RefObject<UnifiedComposerHandle | null>,
) {
  const navigate = useNavigate();
  const sessionMatch = useMatch("/sessions/:sessionId");
  const currentSessionId = sessionMatch?.params.sessionId;

  // A-A chord state
  const lastAPress = useRef<number>(0);

  useEffect(() => {
    const orderedSessions = sortSessions(allSessions);
    const currentSession = currentSessionId
      ? allSessions.find((s) => s.slug === currentSessionId)
      : undefined;

    function handler(e: KeyboardEvent) {
      // Alt+Escape: stop agents — works globally, even in text inputs
      if (e.key === "Escape" && e.altKey && currentSession) {
        e.preventDefault();
        void (async () => {
          const client = await getShipClient();
          await client.stopAgents(currentSession.id);
        })();
        return;
      }

      // Everything below is gated on not being in an editable target
      if (isEditableTarget(e.target)) return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;

      // J/K: cycle sessions
      if (e.key === "j" || e.key === "k") {
        if (orderedSessions.length === 0) return;
        const idx = currentSessionId
          ? orderedSessions.findIndex((s) => s.slug === currentSessionId)
          : -1;

        let next: number;
        if (e.key === "j") {
          // Next session (down in the sidebar)
          next = idx >= orderedSessions.length - 1 ? 0 : idx + 1;
        } else {
          // Previous session (up in the sidebar)
          next = idx <= 0 ? orderedSessions.length - 1 : idx - 1;
        }
        e.preventDefault();
        navigate(`/sessions/${orderedSessions[next].slug}`);
        return;
      }

      // Space: toggle voice recording
      if (e.key === " " && composerRef?.current) {
        e.preventDefault();
        if (composerRef.current.isRecording()) {
          composerRef.current.stopRecording();
        } else {
          composerRef.current.startRecording();
        }
        return;
      }

      // A-A chord: archive current session
      if (e.key === "a" && currentSession) {
        const now = Date.now();
        if (now - lastAPress.current < 500) {
          e.preventDefault();
          lastAPress.current = 0;
          void (async () => {
            const client = await getShipClient();
            const result = await client.archiveSession({ id: currentSession.id, force: true });
            if (result.tag === "Archived") {
              if (currentSessionId) onSessionArchived?.(currentSessionId);
              navigate("/");
            }
          })();
        } else {
          lastAPress.current = now;
        }
        return;
      }
    }

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [allSessions, currentSessionId, navigate, onSessionArchived]);
}
