import { Routes, Route, Link, useMatch } from "react-router-dom";
import { Flex, Box, Text, IconButton } from "@radix-ui/themes";
import { SpeakerHigh, SpeakerSlash } from "@phosphor-icons/react";
import { SessionListPage } from "./pages/SessionListPage";
import { SessionViewPage } from "./pages/SessionViewPage";
import { ConnectionBanner } from "./components/ConnectionBanner";
import { NotificationPrompt } from "./components/NotificationPrompt";
import { SessionSidebar } from "./components/SessionSidebar";
import { useSoundEnabled } from "./context/SoundContext";
import { useSessionList } from "./hooks/useSessionList";

// r[ui.layout.shell]
export function App() {
  const sessionMatch = useMatch("/sessions/:sessionId");
  const currentSessionId = sessionMatch?.params.sessionId;
  const inSessionView = !!sessionMatch;
  const { soundEnabled, setSoundEnabled } = useSoundEnabled();
  const allSessions = useSessionList();

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
            <IconButton
              variant="ghost"
              size="2"
              onClick={() => setSoundEnabled(!soundEnabled)}
              aria-label={soundEnabled ? "Mute sounds" : "Unmute sounds"}
            >
              {soundEnabled ? <SpeakerHigh size={18} /> : <SpeakerSlash size={18} />}
            </IconButton>
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

      <Flex style={{ flex: 1, overflow: "hidden", minHeight: 0 }}>
        {allSessions.length > 0 && (
          <SessionSidebar sessions={allSessions} currentSessionId={currentSessionId} />
        )}
        <Box
          style={{
            flex: 1,
            overflow: inSessionView ? "hidden" : "auto",
            display: "flex",
            flexDirection: "column",
          }}
        >
          <Routes>
            <Route path="/" element={<SessionListPage />} />
            <Route path="/sessions/:sessionId" element={<SessionViewPage />} />
          </Routes>
        </Box>
      </Flex>
    </Flex>
  );
}
