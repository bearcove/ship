import { useState } from "react";
import {
  Badge,
  Box,
  Button,
  Callout,
  DataList,
  Dialog,
  Flex,
  IconButton,
  Popover,
  Text,
  TextArea,
  Tooltip,
} from "@radix-ui/themes";
import { ClockCounterClockwise } from "@phosphor-icons/react";
import type { SessionStartupState, TaskRecord, TaskStatus } from "../generated/ship";
import { useTaskHistory } from "../hooks/useTaskHistory";
import { getShipClient } from "../api/client";
import { taskBar, taskDescription } from "../styles/session-view.css";

const STATUS_COLOR: Record<
  TaskStatus["tag"],
  "gray" | "blue" | "amber" | "orange" | "green" | "red"
> = {
  Assigned: "gray",
  Working: "blue",
  ReviewPending: "amber",
  SteerPending: "orange",
  Accepted: "green",
  Cancelled: "red",
};

// r[ui.task-bar.history]
function TaskHistoryPopover({ sessionId }: { sessionId: string }) {
  const history = useTaskHistory(sessionId);

  return (
    <Popover.Root>
      <Popover.Trigger>
        <IconButton variant="ghost" size="2" aria-label="Task history">
          <ClockCounterClockwise size={16} />
        </IconButton>
      </Popover.Trigger>
      <Popover.Content side="top" align="end" style={{ maxWidth: 480 }}>
        <Text size="2" weight="medium" mb="2" as="p">
          Task History
        </Text>
        {history.length === 0 ? (
          <Text size="2" color="gray">
            No completed tasks yet.
          </Text>
        ) : (
          <DataList.Root size="1" orientation="vertical">
            {history.map((task) => (
              <DataList.Item key={task.id}>
                <DataList.Label>
                  <Badge color={STATUS_COLOR[task.status.tag]} size="1">
                    {task.status.tag}
                  </Badge>
                </DataList.Label>
                <DataList.Value>
                  <Text size="2">{task.description}</Text>
                </DataList.Value>
              </DataList.Item>
            ))}
          </DataList.Root>
        )}
      </Popover.Content>
    </Popover.Root>
  );
}

// r[ui.task-bar.new-task]
// r[proto.assign]
// r[task.assign]
function NewTaskDialog({
  sessionId,
  startupState,
}: {
  sessionId: string;
  startupState: SessionStartupState | null;
}) {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleAssign() {
    if (!text.trim() || loading || startupState?.tag !== "Ready") return;
    setLoading(true);
    setError(null);
    try {
      const client = await getShipClient();
      const result = await client.assign(sessionId, text);
      if (result.tag === "Failed") {
        setError(result.message);
        return;
      }
      setOpen(false);
      setText("");
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger>
        <Button size="2" disabled={startupState?.tag !== "Ready"}>
          New Task
        </Button>
      </Dialog.Trigger>
      <Dialog.Content maxWidth="480px">
        <Dialog.Title>Assign New Task</Dialog.Title>
        <Dialog.Description size="2" color="gray">
          Describe the task to assign to the Captain and Mate.
        </Dialog.Description>
        {error && (
          <Callout.Root color="red" mt="3">
            <Callout.Text>{error}</Callout.Text>
          </Callout.Root>
        )}
        <Flex direction="column" gap="3" mt="2">
          <TextArea
            placeholder="Describe the task…"
            value={text}
            onChange={(e) => setText(e.target.value)}
            rows={5}
          />
          <Flex gap="2" justify="end">
            <Dialog.Close>
              <Button variant="soft" color="gray" disabled={loading}>
                Cancel
              </Button>
            </Dialog.Close>
            <Button disabled={!text.trim()} loading={loading} onClick={handleAssign}>
              Assign
            </Button>
          </Flex>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}

interface Props {
  sessionId: string;
  startupState: SessionStartupState | null;
  task: TaskRecord | null;
}

// r[ui.task-bar.layout]
// r[proto.accept]
// r[proto.cancel]
// r[task.accept]
// r[task.cancel]
export function TaskBar({ sessionId, startupState, task }: Props) {
  const [actionLoading, setActionLoading] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  async function handleAccept() {
    if (actionLoading) return;
    setActionLoading(true);
    setActionError(null);
    try {
      const client = await getShipClient();
      await client.accept(sessionId);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : String(error));
    } finally {
      setActionLoading(false);
    }
  }

  async function handleCancel() {
    if (actionLoading) return;
    setActionLoading(true);
    setActionError(null);
    try {
      const client = await getShipClient();
      await client.cancel(sessionId);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : String(error));
    } finally {
      setActionLoading(false);
    }
  }

  return (
    <Flex className={taskBar} align="center" gap="3">
      {task ? (
        <>
          <Tooltip content={task.description}>
            <Box className={taskDescription}>
              <Text size="2">{task.description}</Text>
            </Box>
          </Tooltip>
          <Badge color={STATUS_COLOR[task.status.tag]} size="1" style={{ flexShrink: 0 }}>
            {task.status.tag}
          </Badge>
        </>
      ) : (
        <Text size="2" color="gray" style={{ flex: 1 }}>
          No active task
        </Text>
      )}

      {/* r[ui.task-bar.actions] */}
      <Flex direction="column" align="end" gap="2" style={{ flexShrink: 0 }}>
        {actionError && (
          <Text size="1" color="red">
            {actionError}
          </Text>
        )}
        <Flex gap="2" align="center">
          {task?.status.tag === "Working" && (
            <Button
              size="2"
              color="red"
              variant="soft"
              loading={actionLoading}
              onClick={handleCancel}
            >
              Cancel task
            </Button>
          )}
          {task?.status.tag === "ReviewPending" && (
            <>
              <Button size="2" color="green" loading={actionLoading} onClick={handleAccept}>
                Accept mate work
              </Button>
              <Button
                size="2"
                color="red"
                variant="soft"
                loading={actionLoading}
                onClick={handleCancel}
              >
                Cancel task
              </Button>
            </>
          )}
          {task?.status.tag === "SteerPending" && (
            <Button
              size="2"
              color="red"
              variant="soft"
              loading={actionLoading}
              onClick={handleCancel}
            >
              Cancel task
            </Button>
          )}
          {!task && <NewTaskDialog sessionId={sessionId} startupState={startupState} />}
          <TaskHistoryPopover sessionId={sessionId} />
        </Flex>
      </Flex>
    </Flex>
  );
}
