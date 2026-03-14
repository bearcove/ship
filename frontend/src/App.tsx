import { useCallback, useEffect, useRef, useState } from "react";
import { Routes, Route, useMatch } from "react-router-dom";
import { Flex, Box, IconButton } from "@radix-ui/themes";
import { List } from "@phosphor-icons/react";
import { SessionListPage } from "./pages/SessionListPage";
import { SessionViewPage } from "./pages/SessionViewPage";
import { ConnectionBanner } from "./components/ConnectionBanner";
import { ConnectingSplash } from "./components/ConnectingSplash";
import { ReconnectingIndicator } from "./components/ReconnectingIndicator";
import { WrongPortMessage } from "./components/WrongPortMessage";
import { NotificationPrompt } from "./components/NotificationPrompt";
import { SessionSidebar } from "./components/SessionSidebar";
import { useSessionList } from "./hooks/useSessionList";
import { useGlobalKeyboard } from "./hooks/useGlobalKeyboard";
import { getConnectionState, onConnectionStateChanged } from "./api/client";
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
  const [debugMode, setDebugMode] = useState(readDebugPreference);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [connState, setConnState] = useState(() => getConnectionState());
  const hasEverConnected = useRef(connState === "connected");

  useGlobalKeyboard(allSessions);

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
                <SessionViewPage debugMode={debugMode} allSessions={allSessions} onOpenSidebar={() => setSidebarOpen(true)} />
              }
            />
          </Routes>
        </Box>
        <Box className={appColRight} />
      </Box>

      {connState === "reconnecting" && hasEverConnected.current && <ReconnectingIndicator />}
    </Flex>
  );
}
