import {
  Box,
  Button,
  Code,
  Dialog,
  Flex,
  Text,
} from "@radix-ui/themes";
import type { SessionSummary } from "../generated/ship";

// r[proto.archive-session]
export function ArchiveSessionDialog({
  session,
  unmergedCommits,
  onConfirm,
  onCancel,
  archiving,
}: {
  session: SessionSummary;
  unmergedCommits: string[];
  onConfirm: () => void;
  onCancel: () => void;
  archiving: boolean;
}) {
  return (
    <Dialog.Root open onOpenChange={(open) => !open && onCancel()}>
      <Dialog.Content maxWidth="500px">
        <Dialog.Title>Archive session?</Dialog.Title>
        <Dialog.Description size="2" color="gray">
          <Text>
            <Code variant="ghost">{session.branch_name}</Code> has unmerged work. Archive anyway?
          </Text>
        </Dialog.Description>

        <Box mt="3">
          <Text size="2" weight="medium" mb="2" as="p">
            Unmerged commits ({unmergedCommits.length}):
          </Text>
          <Box style={{ maxHeight: 160, overflowY: "auto" }}>
            <Flex direction="column" gap="1">
              {unmergedCommits.map((commit, i) => (
                <Text key={i} size="1" style={{ fontFamily: "monospace", color: "var(--gray-11)" }}>
                  {commit}
                </Text>
              ))}
            </Flex>
          </Box>
        </Box>

        <Flex gap="2" justify="end" mt="4">
          <Button variant="soft" color="gray" onClick={onCancel} disabled={archiving}>
            Cancel
          </Button>
          <Button color="red" onClick={onConfirm} loading={archiving}>
            Archive anyway
          </Button>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}
