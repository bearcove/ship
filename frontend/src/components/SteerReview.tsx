import { useEffect, useRef, useState } from "react";
import { Box, Button, Card, Flex, Text, TextArea } from "@radix-ui/themes";
import ReactMarkdown from "react-markdown";
import type { SteerReview as SteerReviewType } from "../types";
import { steerReviewCard } from "../styles/session-view.css";

interface Props {
  steer: SteerReviewType;
  onSend?: (message: string) => void;
}

// r[ui.steer-review.layout]
export function SteerReview({ steer, onSend }: Props) {
  const [editMode, setEditMode] = useState(false);
  const [editText, setEditText] = useState(steer.captainSteer);
  const editTextRef = useRef(editText);
  editTextRef.current = editText;

  // r[ui.keys.steer-send]
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      if (!((e.metaKey || e.ctrlKey) && e.key === "Enter")) return;
      if (editMode) {
        onSend?.(editTextRef.current);
      } else {
        onSend?.(steer.captainSteer);
      }
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [editMode, onSend, steer.captainSteer]);

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
              onClick={() => {
                setEditText(steer.captainSteer);
                setEditMode(false);
              }}
            >
              Cancel
            </Button>
            <Button size="2" color="blue" onClick={() => onSend?.(editText)}>
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
          <ReactMarkdown>{steer.captainSteer}</ReactMarkdown>
        </Box>
        {/* r[ui.steer-review.actions] */}
        <Flex gap="2" align="center">
          <Button
            size="2"
            color="blue"
            variant="solid"
            onClick={() => onSend?.(steer.captainSteer)}
          >
            Send to Mate
          </Button>
          <Button size="2" color="blue" variant="outline" onClick={() => setEditMode(true)}>
            Edit &amp; Send
          </Button>
          <Button size="2" color="red" variant="soft">
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
