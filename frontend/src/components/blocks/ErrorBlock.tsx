import { Button, Callout } from "@radix-ui/themes";
import { Warning } from "@phosphor-icons/react";
import type { AgentState, ErrorBlock as ErrorBlockType } from "../../types";

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
      {agentState === "error" && (
        <Button size="1" color="red" variant="soft" mt="2">
          Retry
        </Button>
      )}
    </Callout.Root>
  );
}
