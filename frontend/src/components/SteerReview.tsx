import { useEffect, useRef, useState } from "react";
import { Box, Button, Card, Flex, Text, TextArea } from "@radix-ui/themes";
import { getShipClient } from "../api/client";
import { steerReviewCard } from "../styles/session-view.css";

interface Props {
  sessionId: string;
  steerText: string;
  onDismiss?: () => void;
}

// r[ui.steer-review.layout]
export function SteerReview({ sessionId, steerText, onDismiss }: Props) {
  const [editMode, setEditMode] = useState(false);
  const [editText, setEditText] = useState(steerText);
  const editTextRef = useRef(editText);
  editTextRef.current = editText;
  const [loading, setLoading] = useState(false);

  async function handleSend(text: string) {
    if (loading) return;
    setLoading(true);
    try {
      const client = await getShipClient();
      await client.steer(sessionId, text);
    } finally {
      setLoading(false);
    }
  }

  // r[ui.keys.steer-send]
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      if (!((e.metaKey || e.ctrlKey) && e.key === "Enter")) return;
      if (editMode) {
        handleSend(editTextRef.current);
      } else {
        handleSend(steerText);
      }
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [editMode, steerText]);

  if (editMode) {
    return (
      <Card className={steerReviewCard} size="2">
        <Flex direction="column" gap="3">
          <Text size="2" weight="medium">
            Edit steer before sending
          </Text>
          <TextArea
            value={editText}
            onChange={(e) => setEditText(e.target.value)}
            rows={6}
            style={{ fontFamily: "inherit" }}
          />
          {/* r[ui.steer-review.edit-mode] */}
          <Flex gap="2" justify="end">
            <Button
              size="2"
              variant="soft"
              color="gray"
              disabled={loading}
              onClick={() => {
                setEditText(steerText);
                setEditMode(false);
              }}
            >
              Cancel
            </Button>
            <Button size="2" color="blue" loading={loading} onClick={() => handleSend(editText)}>
              Send
            </Button>
          </Flex>
        </Flex>
      </Card>
    );
  }

  return (
    <Card className={steerReviewCard} size="2">
      <Flex direction="column" gap="3">
        <Text size="2" weight="medium" color="amber">
          Captain's steer — awaiting your review
        </Text>
        <Box style={{ fontSize: "var(--font-size-2)" }}>
          <Text size="2">{steerText}</Text>
        </Box>
        {/* r[ui.steer-review.actions] */}
        <Flex gap="2" align="center">
          <Button
            size="2"
            color="blue"
            variant="solid"
            loading={loading}
            onClick={() => handleSend(steerText)}
          >
            Send to Mate
          </Button>
          <Button
            size="2"
            color="blue"
            variant="outline"
            disabled={loading}
            onClick={() => setEditMode(true)}
          >
            Edit &amp; Send
          </Button>
          <Button
            size="2"
            color="red"
            variant="soft"
            disabled={loading}
            onClick={() => onDismiss?.()}
          >
            Discard
          </Button>
          <Text size="1" color="gray" style={{ marginLeft: "auto" }}>
            ⌘↵ to send
          </Text>
        </Flex>
      </Flex>
    </Card>
  );
}
