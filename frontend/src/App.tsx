import { useCallback, useEffect, useState } from "react";
import { Routes, Route, Link, useMatch } from "react-router-dom";
import { Flex, Box, Text, IconButton } from "@radix-ui/themes";
import { List, SpeakerHigh, SpeakerSlash } from "@phosphor-icons/react";
import { SessionListPage } from "./pages/SessionListPage";
import { SessionAgentRail, SessionViewPage } from "./pages/SessionViewPage";
import { ConnectionBanner } from "./components/ConnectionBanner";
import { NotificationPrompt } from "./components/NotificationPrompt";
import { SessionSidebar } from "./components/SessionSidebar";
import { useSoundEnabled } from "./context/SoundContext";
import { useSessionList } from "./hooks/useSessionList";
import { useProjects } from "./hooks/useProjects";
import {
  appColumns,
  appColLeft,
  appColCenter,
  appColRight,
  hamburgerBtn,
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
  const { soundEnabled, setSoundEnabled } = useSoundEnabled();
  const allSessions = useSessionList();
  const allProjects = useProjects();
  const [debugMode, setDebugMode] = useState(readDebugPreference);
  const [sidebarOpen, setSidebarOpen] = useState(false);

  useEffect(() => {
    writeDebugPreference(debugMode);
  }, [debugMode]);

  const toggleDebug = useCallback(() => setDebugMode((v) => !v), []);

  return (
    <Flex direction="column" style={{ height: "100vh" }}>
      {!inSessionView && (
        <Box
          px="4"
          py="2"
          style={{
            borderBottom: "1px solid var(--gray-a5)",
            flexShrink: 0,
          }}
        >
          <Flex align="center" justify="between">
            <Link to="/" style={{ textDecoration: "none", color: "inherit" }}>
              <Text size="3" weight="bold">
                Ship
              </Text>
            </Link>
            <Flex align="center" gap="1">
              <IconButton
                className={hamburgerBtn}
                variant="ghost"
                size="2"
                onClick={() => setSidebarOpen(true)}
                aria-label="Open sidebar"
              >
                <List size={18} />
              </IconButton>
              <IconButton
                variant="ghost"
                size="2"
                onClick={() => setSoundEnabled(!soundEnabled)}
                aria-label={soundEnabled ? "Mute sounds" : "Unmute sounds"}
              >
                {soundEnabled ? <SpeakerHigh size={18} /> : <SpeakerSlash size={18} />}
              </IconButton>
            </Flex>
          </Flex>
        </Box>
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
