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
import type { Task, TaskStatus } from "../types";
import { useTaskHistory } from "../hooks/useTaskHistory";
import { taskBar, taskDescription } from "../styles/session-view.css";

const STATUS_COLOR: Record<TaskStatus, "gray" | "blue" | "amber" | "orange" | "green" | "red"> = {
  Assigned: "gray",
  Working: "blue",
  ReviewPending: "amber",
  SteerPending: "orange",
  Accepted: "green",
  Cancelled: "red",
};

function relativeTime(date: Date): string {
  const diffMs = Date.now() - date.getTime();
  const diffMin = Math.floor(diffMs / 60_000);
  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  return `${Math.floor(diffHr / 24)}d ago`;
}

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
                  <Badge color={STATUS_COLOR[task.status]} size="1">
                    {task.status}
                  </Badge>
                </DataList.Label>
                <DataList.Value>
                  <Flex direction="column" gap="1">
                    <Text size="2">{task.description}</Text>
                    {task.completedAt && (
                      <Text size="1" color="gray">
                        {relativeTime(task.completedAt)}
                      </Text>
                    )}
                  </Flex>
                </DataList.Value>
              </DataList.Item>
            ))}
          </DataList.Root>
        )}
      </Popover.Content>
    </Popover.Root>
  );
}

function OwnSteerDialog() {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger>
        <Button size="2" variant="outline" color="gray">
          Write your own steer
        </Button>
      </Dialog.Trigger>
      <Dialog.Content maxWidth="480px">
        <Dialog.Title>Steer the Mate directly</Dialog.Title>
        <Flex direction="column" gap="3" mt="2">
          <TextArea
            placeholder="Write instructions for the mate…"
            value={text}
            onChange={(e) => setText(e.target.value)}
            rows={5}
          />
          <Flex gap="2" justify="end">
            <Dialog.Close>
              <Button variant="soft" color="gray">
                Cancel
              </Button>
            </Dialog.Close>
            <Dialog.Close>
              <Button disabled={!text.trim()}>Send</Button>
            </Dialog.Close>
          </Flex>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}

function NewTaskDialog() {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger>
        <Button size="2" color="blue">
          New Task
        </Button>
      </Dialog.Trigger>
      <Dialog.Content maxWidth="480px">
        <Dialog.Title>Assign New Task</Dialog.Title>
        <Flex direction="column" gap="3" mt="2">
          <TextArea
            placeholder="Describe the task…"
            value={text}
            onChange={(e) => setText(e.target.value)}
            rows={5}
          />
          <Flex gap="2" justify="end">
            <Dialog.Close>
              <Button variant="soft" color="gray">
                Cancel
              </Button>
            </Dialog.Close>
            <Dialog.Close>
              <Button disabled={!text.trim()}>Assign</Button>
            </Dialog.Close>
          </Flex>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}

interface Props {
  sessionId: string;
  task?: Task;
}

export function TaskBar({ sessionId, task }: Props) {
  return (
    <Flex className={taskBar} align="center" gap="3">
      {task ? (
        <>
          <Tooltip content={task.description}>
            <Box className={taskDescription}>
              <Text size="2">{task.description}</Text>
            </Box>
          </Tooltip>
          <Badge color={STATUS_COLOR[task.status]} size="1" style={{ flexShrink: 0 }}>
            {task.status}
          </Badge>
        </>
      ) : (
        <Text size="2" color="gray" style={{ flex: 1 }}>
          No active task
        </Text>
      )}

      <Flex gap="2" align="center" style={{ flexShrink: 0 }}>
        <OwnSteerDialog />
        {task?.status === "Working" && (
          <Button size="2" color="red" variant="soft">
            Cancel
          </Button>
        )}
        {task?.status === "ReviewPending" && (
          <>
            <Button size="2" color="green">
              Accept
            </Button>
            <Button size="2" color="red" variant="soft">
              Cancel
            </Button>
          </>
        )}
        {!task && <NewTaskDialog />}
        <TaskHistoryPopover sessionId={sessionId} />
      </Flex>
    </Flex>
  );
}
