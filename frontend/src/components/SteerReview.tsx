import { useState } from "react";
import { Box, Button, Card, Flex, Text, TextArea } from "@radix-ui/themes";
import ReactMarkdown from "react-markdown";
import type { SteerReview as SteerReviewType } from "../types";
import { steerReviewCard } from "../styles/session-view.css";

interface Props {
  steer: SteerReviewType;
}

export function SteerReview({ steer }: Props) {
  const [editMode, setEditMode] = useState(false);
  const [editText, setEditText] = useState(steer.captainSteer);

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
            <Button size="2" color="blue">
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
        <Flex gap="2">
          <Button size="2" color="blue" variant="solid">
            Send to Mate
          </Button>
          <Button size="2" color="blue" variant="outline" onClick={() => setEditMode(true)}>
            Edit &amp; Send
          </Button>
          <Button size="2" color="red" variant="soft">
            Discard
          </Button>
        </Flex>
      </Flex>
    </Card>
  );
}
