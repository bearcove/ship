import { useState, useMemo } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
  Badge,
  Box,
  Button,
  Callout,
  Card,
  Dialog,
  Flex,
  Select,
  SegmentedControl,
  Text,
  TextArea,
  TextField,
  Tooltip,
} from "@radix-ui/themes";
import { WarningCircle, Plus } from "@phosphor-icons/react";
import { useProjects } from "../hooks/useProjects";
import { useSessionList } from "../hooks/useSessionList";
import { useAgentDiscovery } from "../hooks/useAgentDiscovery";
import { useBranches } from "../hooks/useBranches";
import { sessionCard } from "../styles/session-list.css";
import type { AgentKind, TaskStatus } from "../generated/ship";

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

function AgentKindLabel({ kind }: { kind: AgentKind }) {
  return (
    <Badge color={kind.tag === "Claude" ? "violet" : "cyan"} variant="soft" size="1">
      {kind.tag}
    </Badge>
  );
}

function DisabledTooltip({
  content,
  children,
}: {
  content: string | undefined;
  children: React.ReactElement;
}) {
  if (!content) return children;
  return <Tooltip content={content}>{children}</Tooltip>;
}

function AgentKindControl({
  label,
  value,
  onChange,
  claudeAvailable,
  codexAvailable,
}: {
  label: string;
  value: AgentKind;
  onChange: (v: AgentKind) => void;
  claudeAvailable: boolean;
  codexAvailable: boolean;
}) {
  return (
    <Flex direction="column" gap="1">
      <Text size="2" weight="medium">
        {label}
      </Text>
      <SegmentedControl.Root
        value={value.tag}
        onValueChange={(v) => onChange({ tag: v as "Claude" | "Codex" })}
        size="2"
      >
        <DisabledTooltip
          content={claudeAvailable ? undefined : "claude-agent-acp not found on PATH"}
        >
          <SegmentedControl.Item
            value="Claude"
            style={claudeAvailable ? undefined : { opacity: 0.4, pointerEvents: "none" }}
          >
            Claude
          </SegmentedControl.Item>
        </DisabledTooltip>
        <DisabledTooltip content={codexAvailable ? undefined : "codex-acp not found on PATH"}>
          <SegmentedControl.Item
            value="Codex"
            style={codexAvailable ? undefined : { opacity: 0.4, pointerEvents: "none" }}
          >
            Codex
          </SegmentedControl.Item>
        </DisabledTooltip>
      </SegmentedControl.Root>
    </Flex>
  );
}

function BranchCombobox({
  projectName,
  value,
  onChange,
}: {
  projectName: string;
  value: string;
  onChange: (v: string) => void;
}) {
  const branches = useBranches(projectName);
  const [query, setQuery] = useState(value);
  const [open, setOpen] = useState(false);

  const filtered = useMemo(
    () => branches.filter((b) => b.toLowerCase().includes(query.toLowerCase())).slice(0, 8),
    [branches, query],
  );

  return (
    <Flex direction="column" gap="1" style={{ position: "relative" }}>
      <Text size="2" weight="medium">
        Base branch
      </Text>
      <TextField.Root
        placeholder="Filter branches…"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        onBlur={() => setTimeout(() => setOpen(false), 150)}
      />
      {open && filtered.length > 0 && (
        <Box
          style={{
            position: "absolute",
            top: "100%",
            left: 0,
            right: 0,
            zIndex: 50,
            background: "var(--color-panel-solid)",
            border: "1px solid var(--gray-a6)",
            borderRadius: "var(--radius-3)",
            boxShadow: "var(--shadow-4)",
            marginTop: 2,
            overflow: "hidden",
          }}
        >
          {filtered.map((branch) => (
            <Box
              key={branch}
              px="3"
              py="2"
              style={{
                cursor: "pointer",
                background: branch === value ? "var(--accent-a3)" : undefined,
              }}
              onMouseDown={() => {
                onChange(branch);
                setQuery(branch);
                setOpen(false);
              }}
            >
              <Text size="2" style={{ fontFamily: "monospace" }}>
                {branch}
              </Text>
            </Box>
          ))}
        </Box>
      )}
    </Flex>
  );
}

// r[session.create]
function NewSessionDialog({
  open,
  onOpenChange,
  preselectedProject,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  preselectedProject?: string;
}) {
  const projects = useProjects().filter((p) => p.valid);
  const discovery = useAgentDiscovery();

  const defaultProject = preselectedProject ?? (projects.length === 1 ? projects[0].name : "");
  const [projectName, setProjectName] = useState(defaultProject);
  const [captainKind, setCaptainKind] = useState<AgentKind>({ tag: "Claude" });
  const [mateKind, setMateKind] = useState<AgentKind>({ tag: "Claude" });
  const [branch, setBranch] = useState("main");
  const [taskDescription, setTaskDescription] = useState("");

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Content key={String(open)} maxWidth="480px">
        <Dialog.Title>New Session</Dialog.Title>
        <Flex direction="column" gap="4" mt="2">
          <Flex direction="column" gap="1">
            <Text size="2" weight="medium">
              Project
            </Text>
            <Select.Root value={projectName} onValueChange={setProjectName}>
              <Select.Trigger placeholder="Select a project…" />
              <Select.Content>
                {projects.map((p) => (
                  <Select.Item key={p.name} value={p.name}>
                    {p.name}
                  </Select.Item>
                ))}
              </Select.Content>
            </Select.Root>
          </Flex>

          <AgentKindControl
            label="Captain"
            value={captainKind}
            onChange={setCaptainKind}
            claudeAvailable={discovery.claude}
            codexAvailable={discovery.codex}
          />

          <AgentKindControl
            label="Mate"
            value={mateKind}
            onChange={setMateKind}
            claudeAvailable={discovery.claude}
            codexAvailable={discovery.codex}
          />

          <BranchCombobox projectName={projectName} value={branch} onChange={setBranch} />

          <Flex direction="column" gap="1">
            <Text size="2" weight="medium">
              Task description
            </Text>
            <TextArea
              placeholder="Describe the task for the captain and mate…"
              value={taskDescription}
              onChange={(e) => setTaskDescription(e.target.value)}
              rows={4}
            />
          </Flex>

          <Flex gap="2" justify="end" mt="1">
            <Dialog.Close>
              <Button variant="soft" color="gray">
                Cancel
              </Button>
            </Dialog.Close>
            <Dialog.Close>
              <Button disabled={!projectName || !taskDescription.trim()}>Create Session</Button>
            </Dialog.Close>
          </Flex>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}

function AddProjectDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const [path, setPath] = useState("");
  const isInvalid = path.toLowerCase().includes("invalid");

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Content key={String(open)} maxWidth="440px">
        <Dialog.Title>Add Project</Dialog.Title>
        <Flex direction="column" gap="4" mt="2">
          <Flex direction="column" gap="1">
            <Text size="2" weight="medium">
              Repository path
            </Text>
            <TextField.Root
              placeholder="/absolute/path/to/repo"
              value={path}
              onChange={(e) => setPath(e.target.value)}
            />
          </Flex>

          {isInvalid && (
            <Callout.Root color="red" size="1">
              <Callout.Icon>
                <WarningCircle size={16} />
              </Callout.Icon>
              <Callout.Text>Path does not exist or is not a git repository.</Callout.Text>
            </Callout.Root>
          )}

          <Flex gap="2" justify="end" mt="1">
            <Dialog.Close>
              <Button variant="soft" color="gray">
                Cancel
              </Button>
            </Dialog.Close>
            <Button disabled={!path.trim() || isInvalid}>Add</Button>
          </Flex>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}

// r[ui.session-list.layout]
export function SessionListPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const projectFilter = searchParams.get("project") ?? undefined;

  const allProjects = useProjects();
  const validProjects = allProjects.filter((p) => p.valid);
  const sessions = useSessionList(projectFilter);

  const [newSessionOpen, setNewSessionOpen] = useState(false);
  const [addProjectOpen, setAddProjectOpen] = useState(false);

  const noProjects = validProjects.length === 0;

  return (
    <Box p="4" style={{ maxWidth: 720, margin: "0 auto" }}>
      <Flex align="center" justify="between" mb="4">
        <Text size="5" weight="bold">
          Sessions
        </Text>
        <Flex gap="2">
          <Button variant="soft" size="2" onClick={() => setAddProjectOpen(true)}>
            <Plus size={16} />
            Add Project
          </Button>
          {!noProjects && (
            <Button size="2" onClick={() => setNewSessionOpen(true)}>
              <Plus size={16} />
              New Session
            </Button>
          )}
        </Flex>
      </Flex>

      {!noProjects && (
        <Flex mb="4" align="center" gap="2">
          <Select.Root
            value={projectFilter ?? "__all__"}
            onValueChange={(v) => {
              if (v === "__all__") {
                setSearchParams({});
              } else {
                setSearchParams({ project: v });
              }
            }}
          >
            <Select.Trigger placeholder="All projects" />
            <Select.Content>
              <Select.Item value="__all__">All projects</Select.Item>
              {validProjects.map((p) => (
                <Select.Item key={p.name} value={p.name}>
                  {p.name}
                </Select.Item>
              ))}
            </Select.Content>
          </Select.Root>
          {allProjects.some((p) => !p.valid) && (
            <Callout.Root color="amber" size="1" style={{ flex: 1 }}>
              <Callout.Icon>
                <WarningCircle size={16} />
              </Callout.Icon>
              <Callout.Text>
                {allProjects
                  .filter((p) => !p.valid)
                  .map((p) => p.name)
                  .join(", ")}{" "}
                {allProjects.filter((p) => !p.valid).length === 1 ? "has" : "have"} an invalid path.
              </Callout.Text>
            </Callout.Root>
          )}
        </Flex>
      )}

      {noProjects ? (
        <Flex justify="center" mt="8">
          <Callout.Root size="2" style={{ maxWidth: 400 }}>
            <Callout.Icon>
              <WarningCircle size={18} />
            </Callout.Icon>
            <Callout.Text>
              No projects registered. Add a git repository to get started.
            </Callout.Text>
            <Box mt="3">
              <Button onClick={() => setAddProjectOpen(true)}>
                <Plus size={16} />
                Add Project
              </Button>
            </Box>
          </Callout.Root>
        </Flex>
      ) : sessions.length === 0 ? (
        <Flex justify="center" mt="8">
          <Callout.Root size="2" style={{ maxWidth: 400 }}>
            <Callout.Text>
              No sessions yet.{" "}
              {projectFilter ? `No sessions in ${projectFilter}.` : "Create one to get started."}
            </Callout.Text>
            <Box mt="3">
              <Button onClick={() => setNewSessionOpen(true)}>
                <Plus size={16} />
                New Session
              </Button>
            </Box>
          </Callout.Root>
        </Flex>
      ) : (
        <Flex direction="column" gap="3">
          {sessions.map((session) => (
            <Link
              key={session.id}
              to={`/sessions/${session.id}`}
              style={{ textDecoration: "none", color: "inherit" }}
            >
              <Card className={sessionCard}>
                <Flex direction="column" gap="2">
                  <Flex align="center" gap="2" wrap="wrap">
                    <Badge color="gray" variant="outline" size="1">
                      {session.project}
                    </Badge>
                    <Text size="2" style={{ fontFamily: "monospace", color: "var(--gray-11)" }}>
                      {session.branch_name}
                    </Text>
                    <Flex gap="1" ml="auto" align="center">
                      {session.task_status && (
                        <Badge color={STATUS_COLOR[session.task_status.tag]} size="1">
                          {session.task_status.tag}
                        </Badge>
                      )}
                    </Flex>
                  </Flex>

                  {session.current_task_description && (
                    <Text size="3" weight="medium" style={{ lineHeight: 1.4 }}>
                      {session.current_task_description.length > 100
                        ? `${session.current_task_description.slice(0, 97)}…`
                        : session.current_task_description}
                    </Text>
                  )}

                  <Flex align="center" gap="2">
                    <Text size="1" color="gray">
                      Captain:
                    </Text>
                    <AgentKindLabel kind={session.captain.kind} />
                    <Text size="1" color="gray">
                      Mate:
                    </Text>
                    <AgentKindLabel kind={session.mate.kind} />
                  </Flex>
                </Flex>
              </Card>
            </Link>
          ))}
        </Flex>
      )}

      <NewSessionDialog
        open={newSessionOpen}
        onOpenChange={setNewSessionOpen}
        preselectedProject={projectFilter}
      />
      <AddProjectDialog open={addProjectOpen} onOpenChange={setAddProjectOpen} />
    </Box>
  );
}
