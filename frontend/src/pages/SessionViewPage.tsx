import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { Badge, Box, Button, Callout, Flex, Switch, Tabs, Text } from "@radix-ui/themes";
import { Clock, X } from "@phosphor-icons/react";
import { useSession } from "../hooks/useSession";
import { AgentPanel } from "../components/AgentPanel";
import { TaskBar } from "../components/TaskBar";
import { SteerReview } from "../components/SteerReview";
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
import type { SessionDetail } from "../generated/ship";

function getIdleMessage(session: SessionDetail): string | null {
  if (session.current_task?.status.tag === "ReviewPending")
    return "Mate has finished — review and accept, reject, or steer.";
  if (session.current_task?.status.tag === "SteerPending")
    return "Captain's steer is ready — review and send to the mate.";
  if (session.mate.state.tag === "AwaitingPermission")
    return "Mate is waiting for permission approval.";
  return null;
}

// r[ui.layout.session-view]
export function SessionViewPage() {
  const { sessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  const session = useSession(sessionId ?? "");
  const [autonomous, setAutonomous] = useState(session.autonomy_mode.tag === "Autonomous");
  const [mobileTab, setMobileTab] = useState<"captain" | "mate">("captain");

  const idle = getIdleMessage(session);

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
            <AgentPanel sessionId={session.id} agent={session.captain} />
          </Box>
          <Box className={panelColumn}>
            <AgentPanel sessionId={session.id} agent={session.mate} />
          </Box>
        </Box>

        <Box className={mobilePanel}>
          {mobileTab === "captain" ? (
            <AgentPanel sessionId={session.id} agent={session.captain} />
          ) : (
            <AgentPanel sessionId={session.id} agent={session.mate} />
          )}
        </Box>
      </Box>

      {session.pending_steer && <SteerReview steer={{ captainSteer: session.pending_steer }} />}

      <TaskBar sessionId={session.id} task={session.current_task} />
    </Flex>
  );
}
