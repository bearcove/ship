import { useState } from "react";
import { Box, Code, Flex, Text } from "@radix-ui/themes";
import type { AgentAcpInfo } from "../generated/ship";

function AcpInfoCard({ role, info }: { role: "Captain" | "Mate"; info: AgentAcpInfo | null }) {
  const [expanded, setExpanded] = useState(true);

  return (
    <Box
      style={{
        border: "1px solid var(--gray-6)",
        borderRadius: "var(--radius-2)",
        overflow: "hidden",
        flex: 1,
        minWidth: 0,
      }}
    >
      <Flex
        align="center"
        justify="between"
        px="2"
        py="1"
        style={{
          background: "var(--gray-3)",
          cursor: "pointer",
          userSelect: "none",
        }}
        onClick={() => setExpanded((v) => !v)}
      >
        <Text size="1" weight="bold" color="gray">
          {role} ACP
        </Text>
        <Text size="1" color="gray">
          {expanded ? "▲" : "▼"}
        </Text>
      </Flex>

      {expanded && (
        <Box px="2" py="2">
          {!info ? (
            <Text size="1" color="gray">
              not connected
            </Text>
          ) : (
            <Flex direction="column" gap="1">
              <InfoRow label="session_id" value={info.acp_session_id} mono />
              <InfoRow
                label="was_resumed"
                value={String(info.was_resumed)}
                highlight={info.was_resumed ? "green" : undefined}
              />
              <InfoRow label="protocol_version" value={String(info.protocol_version)} />
              <InfoRow label="agent_name" value={info.agent_name ?? "—"} />
              <InfoRow label="agent_version" value={info.agent_version ?? "—"} />
              <SectionDivider label="capabilities" />
              <InfoRow
                label="load_session"
                value={boolLabel(info.cap_load_session)}
                highlight={boolColor(info.cap_load_session)}
              />
              <InfoRow
                label="resume_session"
                value={boolLabel(info.cap_resume_session)}
                highlight={boolColor(info.cap_resume_session)}
              />
              <InfoRow
                label="prompt_image"
                value={boolLabel(info.cap_prompt_image)}
                highlight={boolColor(info.cap_prompt_image)}
              />
              <InfoRow
                label="prompt_audio"
                value={boolLabel(info.cap_prompt_audio)}
                highlight={boolColor(info.cap_prompt_audio)}
              />
              <InfoRow
                label="prompt_embedded_context"
                value={boolLabel(info.cap_prompt_embedded_context)}
                highlight={boolColor(info.cap_prompt_embedded_context)}
              />
              <InfoRow
                label="mcp_http"
                value={boolLabel(info.cap_mcp_http)}
                highlight={boolColor(info.cap_mcp_http)}
              />
              <InfoRow
                label="mcp_sse"
                value={boolLabel(info.cap_mcp_sse)}
                highlight={boolColor(info.cap_mcp_sse)}
              />
              {info.last_event_at && (
                <>
                  <SectionDivider label="timing" />
                  <InfoRow label="last_event_at" value={formatTimestamp(info.last_event_at)} />
                </>
              )}
            </Flex>
          )}
        </Box>
      )}
    </Box>
  );
}

function SectionDivider({ label }: { label: string }) {
  return (
    <Flex align="center" gap="1" mt="1">
      <Text
        size="1"
        color="gray"
        style={{
          opacity: 0.6,
          fontWeight: 600,
          textTransform: "uppercase",
          letterSpacing: "0.05em",
        }}
      >
        {label}
      </Text>
      <Box style={{ flex: 1, height: 1, background: "var(--gray-5)" }} />
    </Flex>
  );
}

function InfoRow({
  label,
  value,
  mono,
  highlight,
}: {
  label: string;
  value: string;
  mono?: boolean;
  highlight?: "green" | "red" | "gray";
}) {
  return (
    <Flex align="baseline" gap="2">
      <Text size="1" color="gray" style={{ minWidth: 160, flexShrink: 0, opacity: 0.7 }}>
        {label}
      </Text>
      {mono ? (
        <Code size="1" style={{ wordBreak: "break-all" }}>
          {value}
        </Code>
      ) : (
        <Text size="1" color={highlight ?? undefined} style={{ wordBreak: "break-all" }}>
          {value}
        </Text>
      )}
    </Flex>
  );
}

function boolLabel(v: boolean): string {
  return v ? "yes" : "no";
}

function boolColor(v: boolean): "green" | "gray" {
  return v ? "green" : "gray";
}

function formatTimestamp(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      fractionalSecondDigits: 3,
    });
  } catch {
    return iso;
  }
}

// r[acp.debug-info]
export function SessionDebugPanel({
  captainAcpInfo,
  mateAcpInfo,
}: {
  captainAcpInfo: AgentAcpInfo | null;
  mateAcpInfo: AgentAcpInfo | null;
}) {
  const [panelExpanded, setPanelExpanded] = useState(false);

  return (
    <Box
      style={{
        borderTop: "1px solid var(--gray-5)",
        background: "var(--gray-2)",
        flexShrink: 0,
      }}
    >
      <Flex
        align="center"
        justify="between"
        px="3"
        py="1"
        style={{ cursor: "pointer", userSelect: "none" }}
        onClick={() => setPanelExpanded((v) => !v)}
      >
        <Text size="1" color="gray" weight="bold">
          🛠 ACP Debug Info
        </Text>
        <Text size="1" color="gray">
          {panelExpanded ? "▲ hide" : "▼ show"}
        </Text>
      </Flex>

      {panelExpanded && (
        <Flex gap="2" px="3" pb="3">
          <AcpInfoCard role="Captain" info={captainAcpInfo} />
          <AcpInfoCard role="Mate" info={mateAcpInfo} />
        </Flex>
      )}
    </Box>
  );
}
