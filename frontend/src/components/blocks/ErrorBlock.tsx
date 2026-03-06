import { Button, Callout, Flex } from "@radix-ui/themes";
import { WarningCircle } from "@phosphor-icons/react";
import type { AgentState, ContentBlock } from "../../generated/ship";

type ErrorBlockType = Extract<ContentBlock, { tag: "Error" }>;

interface Props {
  block: ErrorBlockType;
  agentState: AgentState;
}

// r[ui.block.error]
export function ErrorBlock({ block, agentState }: Props) {
  return (
    <Callout.Root color="red" size="1" role="alert">
      <Callout.Icon>
        <WarningCircle size={16} aria-label="Error" />
      </Callout.Icon>
      <Flex direction="column" gap="2">
        <Callout.Text>{block.message}</Callout.Text>
        {agentState.tag === "Error" && (
          <Button size="1" color="red" variant="soft">
            Retry
          </Button>
        )}
      </Flex>
    </Callout.Root>
  );
}
