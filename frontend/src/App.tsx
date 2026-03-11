import { useCallback, useEffect, useState } from "react";
import { Routes, Route, useMatch } from "react-router-dom";
import { Flex, Box, IconButton } from "@radix-ui/themes";
import { List } from "@phosphor-icons/react";
import { SessionListPage } from "./pages/SessionListPage";
import { SessionAgentRail, SessionViewPage } from "./pages/SessionViewPage";
import { ConnectionBanner } from "./components/ConnectionBanner";
import { NotificationPrompt } from "./components/NotificationPrompt";
import { SessionSidebar } from "./components/SessionSidebar";
import { useSessionList } from "./hooks/useSessionList";
import { useProjects } from "./hooks/useProjects";
import {
  appColumns,
  appColLeft,
  appColCenter,
  appColRight,
  floatingHamburger,
} from "./styles/session-view.css";

function readDebugPreference(): boolean {
  try {
    return window.localStorage?.getItem("ship.debug") === "1";
  } catch {
    return false;
  }
}

function writeDebugPreference(enabled: boolean) {
  try {
    window.localStorage?.setItem("ship.debug", enabled ? "1" : "0");
  } catch {
    // ignore
  }
}

// r[ui.layout.shell]
export function App() {
  const sessionMatch = useMatch("/sessions/:sessionId");
  const currentSessionId = sessionMatch?.params.sessionId;
  const inSessionView = !!sessionMatch;
  const allSessions = useSessionList();
  const allProjects = useProjects();
  const [debugMode, setDebugMode] = useState(readDebugPreference);
  const [sidebarOpen, setSidebarOpen] = useState(false);

  useEffect(() => {
    writeDebugPreference(debugMode);
  }, [debugMode]);

  const toggleDebug = useCallback(() => setDebugMode((v) => !v), []);

  useEffect(() => {
    let startX = 0;
    let startY = 0;
    function onTouchStart(e: TouchEvent) {
      startX = e.touches[0].clientX;
      startY = e.touches[0].clientY;
    }
    function onTouchEnd(e: TouchEvent) {
      const dx = e.changedTouches[0].clientX - startX;
      const dy = e.changedTouches[0].clientY - startY;
      if (startX < window.innerWidth / 2 && dx > 60 && Math.abs(dy) < 80) {
        setSidebarOpen(true);
      }
    }
    window.addEventListener("touchstart", onTouchStart, { passive: true });
    window.addEventListener("touchend", onTouchEnd, { passive: true });
    return () => {
      window.removeEventListener("touchstart", onTouchStart);
      window.removeEventListener("touchend", onTouchEnd);
    };
  }, []);

  return (
    <Flex direction="column" style={{ height: "100vh" }}>
      {!inSessionView && (
        <IconButton
          className={floatingHamburger}
          variant="soft"
          color="gray"
          size="2"
          onClick={() => setSidebarOpen(true)}
          aria-label="Open sidebar"
        >
          <List size={18} />
        </IconButton>
      )}
      <ConnectionBanner
        connected={true}
        phase="live"
        disconnectReason={null}
        replayEventCount={0}
        connectionAttempt={0}
        lastSeq={null}
        lastEventKind={null}
      />
      <NotificationPrompt />

      <Box className={appColumns}>
        <Box className={appColLeft}>
          <SessionSidebar
            projects={allProjects}
            sessions={allSessions}
            currentSessionId={currentSessionId}
            debugMode={debugMode}
            onToggleDebug={toggleDebug}
            isOpen={sidebarOpen}
            onClose={() => setSidebarOpen(false)}
          />
        </Box>
        <Box className={appColCenter} style={{ overflow: inSessionView ? "hidden" : "auto" }}>
          <Routes>
            <Route path="/" element={<SessionListPage />} />
            <Route
              path="/sessions/:sessionId"
              element={
                <SessionViewPage debugMode={debugMode} onOpenSidebar={() => setSidebarOpen(true)} />
              }
            />
          </Routes>
        </Box>
        <Box
          className={appColRight}
          style={{ borderLeft: currentSessionId ? "1px solid var(--gray-a5)" : undefined }}
        >
          {currentSessionId ? <SessionAgentRail sessionId={currentSessionId} /> : null}
        </Box>
      </Box>
    </Flex>
  );
}
