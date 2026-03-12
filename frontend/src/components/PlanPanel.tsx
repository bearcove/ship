import { Box, Flex, Spinner, Text } from "@radix-ui/themes";
import { Circle, CheckCircle, XCircle } from "@phosphor-icons/react";
import type { PlanStep, PlanStepStatus } from "../generated/ship";
import { planPanel, planStepRow, planStepText } from "../styles/session-view.css";

function StepIcon({ status }: { status: PlanStepStatus }) {
  switch (status.tag) {
    case "Pending":
      return <Circle size={12} style={{ color: "var(--gray-8)", flexShrink: 0 }} />;
    case "InProgress":
      return <Spinner size="1" />;
    case "Completed":
      return (
        <CheckCircle size={12} weight="fill" style={{ color: "var(--green-9)", flexShrink: 0 }} />
      );
    case "Failed":
      return <XCircle size={12} weight="fill" style={{ color: "var(--red-9)", flexShrink: 0 }} />;
  }
}

interface Props {
  steps: PlanStep[];
}

export function PlanPanel({ steps }: Props) {
  if (steps.length === 0) return null;

  const completed = steps.filter((s) => s.status.tag === "Completed").length;

  return (
    <Box className={planPanel}>
      <Flex align="center" justify="between" mb="2">
        <Text size="1" weight="medium" color="gray">
          Plan
        </Text>
        <Text size="1" color="gray">
          {completed}/{steps.length}
        </Text>
      </Flex>
      <Flex direction="column" gap="1">
        {steps.map((step, i) => (
          <Flex key={i} align="start" gap="2" className={planStepRow}>
            <Box style={{ paddingTop: 2, display: "flex" }}>
              <StepIcon status={step.status} />
            </Box>
            <Text
              size="1"
              className={planStepText}
              style={{
                color:
                  step.status.tag === "Completed"
                    ? "var(--gray-9)"
                    : step.status.tag === "Failed"
                      ? "var(--red-11)"
                      : "var(--gray-12)",
                textDecoration: step.status.tag === "Completed" ? "line-through" : undefined,
              }}
            >
              {step.title || step.description}
            </Text>
          </Flex>
        ))}
      </Flex>
    </Box>
  );
}
