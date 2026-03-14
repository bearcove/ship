import { useEffect, useRef } from "react";
import { useNavigate, useMatch } from "react-router-dom";
import type { SessionSummary } from "../generated/ship";
import { useTranscription } from "../context/TranscriptionContext";
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
) {
  const navigate = useNavigate();
  const sessionMatch = useMatch("/sessions/:sessionId");
  const currentSessionId = sessionMatch?.params.sessionId;
  const transcription = useTranscription();

  // A-A chord state
  const lastAPress = useRef<number>(0);
  // Space hold-to-record: timestamp of keydown, 0 when idle
  const spaceDownAt = useRef<number>(0);

  // Keep stable refs so the effect closure always sees current values
  const transcriptionRef = useRef(transcription);
  transcriptionRef.current = transcription;
  const currentSessionIdRef = useRef(currentSessionId);
  currentSessionIdRef.current = currentSessionId;

  useEffect(() => {
    const orderedSessions = sortSessions(allSessions);
    const currentSession = currentSessionIdRef.current
      ? allSessions.find((s) => s.slug === currentSessionIdRef.current)
      : undefined;

    function handleKeyDown(e: KeyboardEvent) {
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
        const sessionId = currentSessionIdRef.current;
        const idx = sessionId
          ? orderedSessions.findIndex((s) => s.slug === sessionId)
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

      // Space: tap to toggle recording, hold to record-and-send
      if (e.key === " " && !e.repeat && currentSessionIdRef.current) {
        const t = transcriptionRef.current;
        e.preventDefault();
        if (t.isRecording()) {
          t.stopAndSend();
          spaceDownAt.current = 0;
        } else {
          spaceDownAt.current = Date.now();
          t.startRecording(currentSessionIdRef.current);
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
              const sessionId = currentSessionIdRef.current;
              if (sessionId) onSessionArchived?.(sessionId);
              navigate("/");
            }
          })();
        } else {
          lastAPress.current = now;
        }
        return;
      }
    }

    function handleKeyUp(e: KeyboardEvent) {
      if (e.key !== " ") return;
      if (isEditableTarget(e.target)) return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      e.preventDefault();
      const t = transcriptionRef.current;
      if (
        spaceDownAt.current > 0 &&
        t.isRecording() &&
        Date.now() - spaceDownAt.current > 300
      ) {
        t.stopAndSend();
      }
      spaceDownAt.current = 0;
    }

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, [allSessions, navigate, onSessionArchived]);
}
