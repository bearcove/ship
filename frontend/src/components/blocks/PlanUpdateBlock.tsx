import { Card, Text } from "@radix-ui/themes";
import { Circle, SpinnerGap, CheckCircle, XCircle } from "@phosphor-icons/react";
import type { ContentBlock, PlanStepStatus } from "../../generated/ship";

type PlanUpdateBlockType = Extract<ContentBlock, { tag: "PlanUpdate" }>;

interface Props {
  block: PlanUpdateBlockType;
}

function StepIcon({ status }: { status: PlanStepStatus }) {
  switch (status.tag) {
    case "Planned":
      return <Circle size={14} style={{ color: "var(--gray-9)", flexShrink: 0 }} />;
    case "InProgress":
      return (
        <SpinnerGap
          size={14}
          style={{ color: "var(--blue-9)", flexShrink: 0, animation: "spin 1s linear infinite" }}
        />
      );
    case "Completed":
      return (
        <CheckCircle size={14} weight="fill" style={{ color: "var(--green-9)", flexShrink: 0 }} />
      );
    case "Failed":
      return <XCircle size={14} weight="fill" style={{ color: "var(--red-9)", flexShrink: 0 }} />;
  }
}

export function PlanUpdateBlock({ block }: Props) {
  return (
    <Card size="1">
      <ol
        style={{
          margin: 0,
          padding: "0 0 0 var(--space-1)",
          listStyle: "none",
          display: "flex",
          flexDirection: "column",
          gap: "var(--space-1)",
        }}
      >
        {block.steps.map((step, i) => (
          <li key={i} style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
            <StepIcon status={step.status} />
            <Text
              size="1"
              style={{
                color:
                  step.status.tag === "Completed"
                    ? "var(--gray-10)"
                    : step.status.tag === "Failed"
                      ? "var(--red-11)"
                      : "var(--gray-12)",
                textDecoration: step.status.tag === "Completed" ? "line-through" : undefined,
              }}
            >
              {step.description}
            </Text>
          </li>
        ))}
      </ol>
    </Card>
  );
}
