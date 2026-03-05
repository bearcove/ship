import { Button, Callout } from "@radix-ui/themes";
import { Warning } from "@phosphor-icons/react";
import type { AgentState, ContentBlock } from "../../generated/ship";

type ErrorBlockType = Extract<ContentBlock, { tag: "Error" }>;

interface Props {
  block: ErrorBlockType;
  agentState: AgentState;
}

export function ErrorBlock({ block, agentState }: Props) {
  return (
    <Callout.Root color="red" size="1">
      <Callout.Icon>
        <Warning size={16} />
      </Callout.Icon>
      <Callout.Text>{block.message}</Callout.Text>
      {agentState.tag === "Error" && (
        <Button size="1" color="red" variant="soft" mt="2">
          Retry
        </Button>
      )}
    </Callout.Root>
  );
}
