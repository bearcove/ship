import { Box, Flex, Text } from "@radix-ui/themes";
import { Anchor } from "@phosphor-icons/react";
import { Link } from "react-router-dom";
import { useActivityEntries } from "../hooks/useActivityEntries";
import { relativeTime } from "../utils/time";
import { activityBubble, activityTimeline } from "../styles/admiral.css";
import type { ActivityEntry } from "../generated/ship";

function activityLabel(entry: ActivityEntry): string {
  switch (entry.kind.tag) {
    case "CaptainMessage":
      return entry.kind.message;
    case "AdmiralMessage":
      return entry.kind.message;
    case "SessionCreated":
      return "Session created";
    case "SessionArchived":
      return "Session archived";
  }
}

function ActivityBubble({ entry }: { entry: ActivityEntry }) {
  const title = entry.session_title ?? entry.session_slug;
  return (
    <div className={activityBubble}>
      <Flex justify="between" align="center" gap="3">
        <Flex direction="column" gap="1">
          <Text size="2" weight="medium">
            {activityLabel(entry)}
          </Text>
          <Text size="1" color="gray">
            <Link
              to={`/sessions/${entry.session_slug}`}
              style={{ color: "inherit", textDecoration: "underline" }}
            >
              {title}
            </Link>
          </Text>
        </Flex>
        <Text size="1" color="gray" style={{ whiteSpace: "nowrap" }}>
          {relativeTime(entry.timestamp)}
        </Text>
      </Flex>
    </div>
  );
}

export function AdmiralPage() {
  const entries = useActivityEntries();

  return (
    <Flex direction="column" style={{ height: "100%" }}>
      <Box p="4" style={{ borderBottom: "1px solid var(--gray-a4)" }}>
        <Flex align="center" gap="2">
          <Anchor size={20} weight="bold" />
          <Text size="5" weight="bold">
            Admiral
          </Text>
          <Text size="2" color="gray">
            Activity log
          </Text>
        </Flex>
      </Box>
      <div className={activityTimeline}>
        {entries.length === 0 ? (
          <Text size="2" color="gray">
            No activity yet.
          </Text>
        ) : (
          entries.map((entry) => (
            <ActivityBubble key={Number(entry.id)} entry={entry} />
          ))
        )}
      </div>
    </Flex>
  );
}
