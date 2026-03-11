import { useState } from "react";
import { Box, Button, Card, Code, Flex, ScrollArea, Text, TextArea } from "@radix-ui/themes";
import { Copy } from "@phosphor-icons/react";
import { getShipClient } from "../api/client";
import type { HumanReviewRequest } from "../generated/ship";
import { diffAdd, diffContext, diffRemove, humanReviewCard } from "../styles/session-view.css";

interface Props {
  sessionId: string;
  review: HumanReviewRequest;
}

function UnifiedDiffView({ diff }: { diff: string }) {
  return (
    <ScrollArea style={{ maxHeight: "40vh", maxWidth: "100%" }}>
      <Box style={{ fontFamily: "monospace", fontSize: "var(--font-size-1)", whiteSpace: "pre" }}>
        {diff.split("\n").map((line, index) => {
          if (line.startsWith("+") && !line.startsWith("+++")) {
            return (
              <span key={index} className={diffAdd}>
                {line}
              </span>
            );
          }
          if (line.startsWith("-") && !line.startsWith("---")) {
            return (
              <span key={index} className={diffRemove}>
                {line}
              </span>
            );
          }
          return (
            <span key={index} className={diffContext}>
              {line}
            </span>
          );
        })}
      </Box>
    </ScrollArea>
  );
}

// r[ui.human-review.panel]
export function HumanReview({ sessionId, review }: Props) {
  const [mode, setMode] = useState<"review" | "feedback">("review");
  const [feedbackText, setFeedbackText] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  async function sendReply(message: string) {
    if (loading) return;
    setLoading(true);
    setError(null);
    try {
      const client = await getShipClient();
      await client.replyToHuman(sessionId, message);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setLoading(false);
    }
  }

  function copyPath() {
    void navigator.clipboard.writeText(review.worktree_path);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  if (mode === "feedback") {
    return (
      <Card className={humanReviewCard} size="2">
        <Flex direction="column" gap="3">
          <Text size="2" weight="medium">
            Request changes
          </Text>
          <TextArea
            value={feedbackText}
            onChange={(e) => setFeedbackText(e.target.value)}
            placeholder="Describe what needs to change…"
            rows={4}
            style={{ fontFamily: "inherit" }}
            autoFocus
          />
          <Flex gap="2" justify="end">
            <Button
              size="2"
              variant="soft"
              color="gray"
              disabled={loading}
              onClick={() => setMode("review")}
            >
              Cancel
            </Button>
            <Button
              size="2"
              color="blue"
              loading={loading}
              disabled={!feedbackText.trim()}
              onClick={() => sendReply(feedbackText)}
            >
              Send feedback
            </Button>
          </Flex>
          {error && (
            <Text size="1" color="red">
              {error}
            </Text>
          )}
        </Flex>
      </Card>
    );
  }

  return (
    <Card className={humanReviewCard} size="2">
      <Flex direction="column" gap="3">
        <Text size="2" weight="medium" color="blue">
          Captain is asking for your review
        </Text>

        <Text size="2">{review.message}</Text>

        {review.diff && <UnifiedDiffView diff={review.diff} />}

        <Flex align="center" gap="2">
          <Code
            size="1"
            style={{
              flex: 1,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {review.worktree_path}
          </Code>
          <Button size="1" variant="soft" color="gray" onClick={copyPath}>
            <Copy size={12} />
            {copied ? "Copied!" : "Copy path"}
          </Button>
        </Flex>

        <Flex gap="2" align="center">
          <Button
            size="2"
            color="green"
            variant="solid"
            loading={loading}
            onClick={() => sendReply("approved")}
          >
            Approve
          </Button>
          <Button
            size="2"
            color="amber"
            variant="soft"
            disabled={loading}
            onClick={() => setMode("feedback")}
          >
            Request changes
          </Button>
        </Flex>

        {error && (
          <Text size="1" color="red">
            {error}
          </Text>
        )}
      </Flex>
    </Card>
  );
}
