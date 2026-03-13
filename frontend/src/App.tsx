import { useCallback, useEffect, useRef, useState } from "react";
import { Routes, Route, useMatch } from "react-router-dom";
import { Flex, Box, IconButton } from "@radix-ui/themes";
import { List, ListChecks } from "@phosphor-icons/react";
import { SessionListPage } from "./pages/SessionListPage";
import { SessionAgentRail, SessionViewPage } from "./pages/SessionViewPage";
import { ConnectionBanner } from "./components/ConnectionBanner";
import { ConnectingSplash } from "./components/ConnectingSplash";
import { ReconnectingIndicator } from "./components/ReconnectingIndicator";
import { WrongPortMessage } from "./components/WrongPortMessage";
import { NotificationPrompt } from "./components/NotificationPrompt";
import { SessionSidebar } from "./components/SessionSidebar";
import { useSessionList } from "./hooks/useSessionList";
import { useProjects } from "./hooks/useProjects";
import { getConnectionState, onConnectionStateChanged } from "./api/client";
import {
  appColumns,
  appColLeft,
  appColCenter,
  appColRight,
  floatingHamburger,
  floatingTaskBtn,
  taskPanelBackdrop,
  taskPanelRoot,
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
  const [taskPanelOpen, setTaskPanelOpen] = useState(false);
  const [connState, setConnState] = useState(() => getConnectionState());
  const hasEverConnected = useRef(connState === "connected");

  useEffect(() => {
    writeDebugPreference(debugMode);
  }, [debugMode]);

  useEffect(() => {
    return onConnectionStateChanged((state) => {
      setConnState(state);
      if (state === "connected") {
        hasEverConnected.current = true;
      }
    });
  }, []);

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
      // Swipe right from left half → open sidebar
      if (startX < window.innerWidth / 2 && dx > 60 && Math.abs(dy) < 80) {
        setSidebarOpen(true);
      }
      // Swipe left from right half → open task panel
      if (startX > window.innerWidth / 2 && dx < -60 && Math.abs(dy) < 80) {
        setTaskPanelOpen(true);
      }
      // Swipe right while task panel is open → close it
      if (taskPanelOpen && dx > 60 && Math.abs(dy) < 80) {
        setTaskPanelOpen(false);
      }
    }
    window.addEventListener("touchstart", onTouchStart, { passive: true });
    window.addEventListener("touchend", onTouchEnd, { passive: true });
    return () => {
      window.removeEventListener("touchstart", onTouchStart);
      window.removeEventListener("touchend", onTouchEnd);
    };
  }, [taskPanelOpen]);

  if (connState === "wrong-port") {
    return <WrongPortMessage />;
  }

  if (connState === "initial-connecting" && !hasEverConnected.current) {
    return <ConnectingSplash />;
  }

  return (
    <Flex direction="column" style={{ height: "100dvh" }}>
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
      {inSessionView && currentSessionId && (
        <IconButton
          className={floatingTaskBtn}
          variant="soft"
          color="gray"
          size="2"
          onClick={() => setTaskPanelOpen(true)}
          aria-label="Open task panel"
        >
          <ListChecks size={18} />
        </IconButton>
      )}
      {connState !== "reconnecting" && (
        <ConnectionBanner
          connected={connState === "connected"}
          phase="live"
          disconnectReason={null}
          replayEventCount={0}
          connectionAttempt={0}
          lastSeq={null}
          lastEventKind={null}
        />
      )}
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

      {inSessionView && currentSessionId && (
        <>
          {taskPanelOpen && (
            <Box className={taskPanelBackdrop} onClick={() => setTaskPanelOpen(false)} />
          )}
          <Box className={taskPanelRoot} data-open={taskPanelOpen}>
            <SessionAgentRail sessionId={currentSessionId} />
          </Box>
        </>
      )}

      {connState === "reconnecting" && hasEverConnected.current && <ReconnectingIndicator />}
    </Flex>
  );
}
