import { Card, Box, Flex, Spinner, Text } from "@radix-ui/themes";
import { Circle, CheckCircle, XCircle } from "@phosphor-icons/react";
import type { ContentBlock, PlanStepStatus } from "../../generated/ship";

type PlanUpdateBlockType = Extract<ContentBlock, { tag: "PlanUpdate" }>;

interface Props {
  block: PlanUpdateBlockType;
}

function StepIcon({ status }: { status: PlanStepStatus }) {
  switch (status.tag) {
    case "Pending":
      return (
        <Box role="img" aria-label="Pending" style={{ display: "flex", alignItems: "center" }}>
          <Circle size={14} style={{ color: "var(--gray-9)", flexShrink: 0 }} />
        </Box>
      );
    case "InProgress":
      return (
        <Box role="img" aria-label="In progress" style={{ display: "flex", alignItems: "center" }}>
          <Spinner size="1" />
        </Box>
      );
    case "Completed":
      return (
        <Box role="img" aria-label="Completed" style={{ display: "flex", alignItems: "center" }}>
          <CheckCircle size={14} weight="fill" style={{ color: "var(--green-9)", flexShrink: 0 }} />
        </Box>
      );
    case "Failed":
      return (
        <Box role="img" aria-label="Failed" style={{ display: "flex", alignItems: "center" }}>
          <XCircle size={14} weight="fill" style={{ color: "var(--red-9)", flexShrink: 0 }} />
        </Box>
      );
  }
}

// r[ui.block.plan.layout]
export function PlanUpdateBlock({ block }: Props) {
  return (
    <Card size="1">
      <ol
        style={{
          margin: 0,
          padding: 0,
          paddingInlineStart: "var(--space-5)",
          display: "flex",
          flexDirection: "column",
          gap: "var(--space-1)",
        }}
      >
        {block.steps.map((step, i) => (
          <li
            key={i}
            style={{
              fontSize: "var(--font-size-1)",
              paddingInlineStart: "var(--space-1)",
            }}
          >
            <Flex align="center" gap="2" justify="between" style={{ minWidth: 0, width: "100%" }}>
              <Flex align="center" gap="2" style={{ minWidth: 0 }}>
                <Text
                  size="1"
                  style={{
                    color: step.status.tag === "Failed" ? "var(--red-11)" : "var(--gray-12)",
                  }}
                >
                  {step.title || step.description}
                </Text>
              </Flex>
              <StepIcon status={step.status} />
            </Flex>
          </li>
        ))}
      </ol>
    </Card>
  );
}
