import { Routes, Route } from "react-router-dom";
import { Flex, Box, Text, IconButton } from "@radix-ui/themes";
import { SpeakerHigh, SpeakerSlash } from "@phosphor-icons/react";
import { SessionListPage } from "./pages/SessionListPage";
import { SessionViewPage } from "./pages/SessionViewPage";
import { ConnectionBanner } from "./components/ConnectionBanner";
import { NotificationPrompt } from "./components/NotificationPrompt";
import { useSoundEnabled } from "./context/SoundContext";

// r[ui.layout.shell]
export function App() {
  const { soundEnabled, setSoundEnabled } = useSoundEnabled();

  return (
    <Flex direction="column" style={{ height: "100vh" }}>
      <Box
        px="4"
        py="2"
        style={{
          borderBottom: "1px solid var(--gray-a5)",
        }}
      >
        <Flex align="center" justify="between">
          <Text size="3" weight="bold">
            Ship
          </Text>
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

      <Box style={{ flex: 1, overflow: "auto" }}>
        <Routes>
          <Route path="/" element={<SessionListPage />} />
          <Route path="/sessions/:sessionId" element={<SessionViewPage />} />
        </Routes>
      </Box>
    </Flex>
  );
}
