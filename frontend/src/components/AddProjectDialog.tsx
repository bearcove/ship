import { useState } from "react";
import {
  Button,
  Callout,
  Dialog,
  Flex,
  Text,
  TextField,
} from "@radix-ui/themes";
import { WarningCircle } from "@phosphor-icons/react";
import { getShipClient } from "../api/client";

// r[ui.add-project.dialog]
export function AddProjectDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const [path, setPath] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleAdd() {
    if (!path.trim()) return;
    setError(null);
    setSubmitting(true);
    try {
      const client = await getShipClient();
      const result = await client.addProject(path);
      if (!result.valid) {
        setError(result.invalid_reason ?? "Unknown validation error");
        return;
      }
      onOpenChange(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Content key={String(open)} maxWidth="440px">
        <Dialog.Title>Add Project</Dialog.Title>
        <Dialog.Description size="2" color="gray">
          Enter the absolute path to a local git repository to add as a project.
        </Dialog.Description>
        <Flex direction="column" gap="4" mt="2">
          <Flex direction="column" gap="1">
            <Text size="2" weight="medium">
              Repository path
            </Text>
            <TextField.Root
              placeholder="/absolute/path/to/repo"
              value={path}
              onChange={(e) => {
                setPath(e.target.value);
                setError(null);
              }}
            />
          </Flex>

          {error && (
            <Callout.Root color="red" size="1">
              <Callout.Icon>
                <WarningCircle size={16} />
              </Callout.Icon>
              <Callout.Text>{error}</Callout.Text>
            </Callout.Root>
          )}

          <Flex gap="2" justify="end" mt="1">
            <Dialog.Close>
              <Button variant="soft" color="gray">
                Cancel
              </Button>
            </Dialog.Close>
            <Button disabled={!path.trim() || submitting} loading={submitting} onClick={handleAdd}>
              Add
            </Button>
          </Flex>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}
