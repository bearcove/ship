import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { Badge, Box, Button, Callout, Flex, Switch, Tabs, Text } from "@radix-ui/themes";
import { Clock, X } from "@phosphor-icons/react";
import { useSession } from "../hooks/useSession";
import { useSessionState } from "../hooks/useSessionState";
import { AgentHeader } from "../components/AgentHeader";
import { AgentPanel } from "../components/AgentPanel";
import { TaskBar } from "../components/TaskBar";
import { SteerReview } from "../components/SteerReview";
import { ConnectionBanner } from "../components/ConnectionBanner";
import {
  sessionViewRoot,
  sessionTopBar,
  sessionBranch,
  desktopGrid,
  panelColumn,
  mobileTabs,
  mobilePanel,
  idleBanner,
} from "../styles/session-view.css";
import type { TaskStatus } from "../generated/ship";

function getIdleMessage(
  taskStatus: TaskStatus | null,
  mateAwaitingPermission: boolean,
): string | null {
  if (taskStatus?.tag === "ReviewPending")
    return "Mate has finished — review and accept, reject, or steer.";
  if (taskStatus?.tag === "SteerPending")
    return "Captain's steer is ready — review and send to the mate.";
  if (mateAwaitingPermission) return "Mate is waiting for permission approval.";
  return null;
}

// r[view.session]
// r[ui.layout.session-view]
export function SessionViewPage() {
  const { sessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  const session = useSession(sessionId ?? "");
  const eventState = useSessionState(sessionId ?? "");
  const [autonomous, setAutonomous] = useState(session.autonomy_mode.tag === "Autonomous");
  const [mobileTab, setMobileTab] = useState<"captain" | "mate">("captain");

  const mateAwaitingPermission =
    eventState.mate !== null && eventState.mate.state.tag === "AwaitingPermission";
  const idle = getIdleMessage(eventState.currentTaskStatus, mateAwaitingPermission);

  // Use live agent snapshots from event state if available, fall back to session detail
  const captain = eventState.captain ?? session.captain;
  const mate = eventState.mate ?? session.mate;

  // r[ui.keys.nav]
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key === "1") setMobileTab("captain");
      if (e.key === "2") setMobileTab("mate");
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  return (
    <Flex className={sessionViewRoot}>
      <Flex className={sessionTopBar}>
        <Text size="2" weight="bold">
          {session.project}
        </Text>
        <Text className={sessionBranch}>{session.branch_name}</Text>
        {/* r[ui.autonomy.toggle] */}
        <Flex align="center" gap="2">
          <Text size="2">Autonomous</Text>
          <Switch size="2" checked={autonomous} onCheckedChange={setAutonomous} />
          <Badge color={autonomous ? "blue" : "gray"} size="1">
            {autonomous ? "Autonomous" : "Human-in-the-loop"}
          </Badge>
        </Flex>
        <Button
          variant="ghost"
          size="2"
          color="gray"
          onClick={() => navigate("/")}
          aria-label="Close session"
        >
          <X size={16} />
        </Button>
      </Flex>

      <ConnectionBanner connected={eventState.connected} />

      <Box className={mobileTabs} px="3" pt="2">
        <Tabs.Root value={mobileTab} onValueChange={(v) => setMobileTab(v as "captain" | "mate")}>
          <Tabs.List>
            <Tabs.Trigger value="captain">Captain</Tabs.Trigger>
            <Tabs.Trigger value="mate">Mate</Tabs.Trigger>
          </Tabs.List>
        </Tabs.Root>
      </Box>

      {/* r[ui.idle.banner] */}
      {idle && (
        <Callout.Root color="amber" size="1" className={idleBanner}>
          <Callout.Icon>
            <Clock size={14} />
          </Callout.Icon>
          <Callout.Text>{idle}</Callout.Text>
        </Callout.Root>
      )}

      <Box style={{ flex: 1, overflow: "hidden", display: "flex", flexDirection: "column" }}>
        <Box className={desktopGrid} style={{ flex: 1 }}>
          <Box className={panelColumn}>
            <AgentHeader agent={captain} />
            <AgentPanel agent={captain} blocks={eventState.captainBlocks.blocks} />
          </Box>
          <Box className={panelColumn}>
            <AgentHeader agent={mate} />
            <AgentPanel agent={mate} blocks={eventState.mateBlocks.blocks} />
          </Box>
        </Box>

        <Box className={mobilePanel}>
          {mobileTab === "captain" ? (
            <>
              <AgentHeader agent={captain} />
              <AgentPanel agent={captain} blocks={eventState.captainBlocks.blocks} />
            </>
          ) : (
            <>
              <AgentHeader agent={mate} />
              <AgentPanel agent={mate} blocks={eventState.mateBlocks.blocks} />
            </>
          )}
        </Box>
      </Box>

      {session.pending_steer && (
        <SteerReview
          sessionId={session.id}
          steerText={session.pending_steer}
          onDismiss={() => {}}
        />
      )}

      <TaskBar sessionId={session.id} task={session.current_task} />
    </Flex>
  );
}
