import { useState } from "react";
import {
  Badge,
  Box,
  Button,
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
import type { TaskRecord, TaskStatus } from "../generated/ship";
import { useTaskHistory } from "../hooks/useTaskHistory";
import { shipClient } from "../api/client";
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

// r[ui.steer-review.own-steer]
function OwnSteerDialog({ sessionId }: { sessionId: string }) {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");
  const [loading, setLoading] = useState(false);

  async function handleSend() {
    if (!text.trim() || loading) return;
    setLoading(true);
    try {
      const client = await shipClient;
      await client.steer(sessionId, text);
      setOpen(false);
      setText("");
    } finally {
      setLoading(false);
    }
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger>
        <Button size="2" variant="outline" color="gray">
          Write your own steer
        </Button>
      </Dialog.Trigger>
      <Dialog.Content maxWidth="480px">
        <Dialog.Title>Steer the Mate directly</Dialog.Title>
        <Dialog.Description size="2" color="gray">
          Send instructions directly to the Mate for the current task.
        </Dialog.Description>
        <Flex direction="column" gap="3" mt="2">
          <TextArea
            placeholder="Write instructions for the mate…"
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
            <Button disabled={!text.trim()} loading={loading} onClick={handleSend}>
              Send
            </Button>
          </Flex>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}

// r[ui.task-bar.new-task]
function NewTaskDialog({ sessionId }: { sessionId: string }) {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");
  const [loading, setLoading] = useState(false);

  async function handleAssign() {
    if (!text.trim() || loading) return;
    setLoading(true);
    try {
      const client = await shipClient;
      await client.assign(sessionId, text);
      setOpen(false);
      setText("");
    } finally {
      setLoading(false);
    }
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger>
        <Button size="2" color="blue">
          New Task
        </Button>
      </Dialog.Trigger>
      <Dialog.Content maxWidth="480px">
        <Dialog.Title>Assign New Task</Dialog.Title>
        <Dialog.Description size="2" color="gray">
          Describe the task to assign to the Captain and Mate.
        </Dialog.Description>
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
  task: TaskRecord | null;
}

// r[ui.task-bar.layout]
export function TaskBar({ sessionId, task }: Props) {
  const [actionLoading, setActionLoading] = useState(false);

  async function handleAccept() {
    if (actionLoading) return;
    setActionLoading(true);
    try {
      const client = await shipClient;
      await client.accept(sessionId);
    } finally {
      setActionLoading(false);
    }
  }

  async function handleCancel() {
    if (actionLoading) return;
    setActionLoading(true);
    try {
      const client = await shipClient;
      await client.cancel(sessionId);
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
      <Flex gap="2" align="center" style={{ flexShrink: 0 }}>
        <OwnSteerDialog sessionId={sessionId} />
        {task?.status.tag === "Working" && (
          <Button
            size="2"
            color="red"
            variant="soft"
            loading={actionLoading}
            onClick={handleCancel}
          >
            Cancel
          </Button>
        )}
        {(task?.status.tag === "ReviewPending" || task?.status.tag === "SteerPending") && (
          <>
            <Button size="2" color="green" loading={actionLoading} onClick={handleAccept}>
              Accept
            </Button>
            <Button
              size="2"
              color="red"
              variant="soft"
              loading={actionLoading}
              onClick={handleCancel}
            >
              Cancel
            </Button>
          </>
        )}
        {!task && <NewTaskDialog sessionId={sessionId} />}
        <TaskHistoryPopover sessionId={sessionId} />
      </Flex>
    </Flex>
  );
}
