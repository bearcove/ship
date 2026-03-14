import { useState, useMemo, useEffect, useRef } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import {
  Badge,
  Box,
  Button,
  Callout,
  Card,
  Code,
  Dialog,
  Flex,
  IconButton,
  Select,
  SegmentedControl,
  Text,
  TextField,
  Tooltip,
} from "@radix-ui/themes";
import { Archive, CaretDown, Plus, WarningCircle } from "@phosphor-icons/react";
import { useProjects } from "../hooks/useProjects";
import { refreshSessionList, useSessionList } from "../hooks/useSessionList";
import { useAgentDiscovery } from "../hooks/useAgentDiscovery";
import { useBranches } from "../hooks/useBranches";
import {
  branchComboboxItem,
  branchComboboxList,
  keyboardShortcutKey,
  sessionCard,
} from "../styles/session-list.css";
import type { AgentKind, SessionSummary, TaskStatus } from "../generated/ship";
import { getShipClient } from "../api/client";
import { agentKindTooltip, sortSessions } from "./session-list-utils";
import { useWorktreeDiffStats } from "../hooks/useWorktreeDiffStats";

// r[ui.session-list.status-colors]
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
    <Badge
      color={kind.tag === "Claude" ? "violet" : kind.tag === "Codex" ? "cyan" : "green"}
      variant="soft"
      size="1"
    >
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

// r[session.agent.kind]
function AgentKindControl({
  label,
  value,
  onChange,
  claudeAvailable,
  codexAvailable,
  opencodeAvailable,
}: {
  label: string;
  value: AgentKind;
  onChange: (v: AgentKind) => void;
  claudeAvailable: boolean;
  codexAvailable: boolean;
  opencodeAvailable: boolean;
}) {
  const discovery = { claude: claudeAvailable, codex: codexAvailable, opencode: opencodeAvailable };
  return (
    <Flex direction="column" gap="1">
      <Text size="2" weight="medium">
        {label}
      </Text>
      <SegmentedControl.Root
        value={value.tag}
        onValueChange={(v) => onChange({ tag: v as "Claude" | "Codex" | "OpenCode" })}
        size="2"
      >
        <DisabledTooltip content={agentKindTooltip("claude", discovery)}>
          <SegmentedControl.Item
            value="Claude"
            style={claudeAvailable ? undefined : { opacity: 0.4, pointerEvents: "none" }}
          >
            Claude
          </SegmentedControl.Item>
        </DisabledTooltip>
        <DisabledTooltip content={agentKindTooltip("codex", discovery)}>
          <SegmentedControl.Item
            value="Codex"
            style={codexAvailable ? undefined : { opacity: 0.4, pointerEvents: "none" }}
          >
            Codex
          </SegmentedControl.Item>
        </DisabledTooltip>
        <DisabledTooltip content={agentKindTooltip("opencode", discovery)}>
          <SegmentedControl.Item
            value="OpenCode"
            style={opencodeAvailable ? undefined : { opacity: 0.4, pointerEvents: "none" }}
          >
            OpenCode
          </SegmentedControl.Item>
        </DisabledTooltip>
      </SegmentedControl.Root>
    </Flex>
  );
}

function isAgentKindAvailable(
  kind: AgentKind,
  discovery: { claude: boolean; codex: boolean; opencode: boolean },
) {
  if (kind.tag === "Claude") return discovery.claude;
  if (kind.tag === "Codex") return discovery.codex;
  return discovery.opencode;
}

function firstAvailableAgentKind(discovery: {
  claude: boolean;
  codex: boolean;
  opencode: boolean;
}): AgentKind | null {
  if (discovery.claude) {
    return { tag: "Claude" };
  }
  if (discovery.codex) {
    return { tag: "Codex" };
  }
  if (discovery.opencode) {
    return { tag: "OpenCode" };
  }
  return null;
}

// r[ui.session-list.create.branch-filter]
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
  const latestQuery = useRef(query);
  const latestValue = useRef(value);

  const branchOptions = useMemo(() => {
    const uniqueBranches = Array.from(new Set(branches));
    const preferredBranch =
      uniqueBranches.find((branch) => branch === "main") ??
      uniqueBranches.find((branch) => branch === "master") ??
      uniqueBranches[0];

    return { uniqueBranches, preferredBranch };
  }, [branches]);

  useEffect(() => {
    setQuery(value);
  }, [value]);

  useEffect(() => {
    latestQuery.current = query;
  }, [query]);

  useEffect(() => {
    latestValue.current = value;
  }, [value]);

  useEffect(() => {
    if (!projectName) {
      setQuery("");
      return;
    }

    if (!value && !query && branchOptions.preferredBranch) {
      onChange(branchOptions.preferredBranch);
      return;
    }

    if (value && !branchOptions.uniqueBranches.includes(value) && branchOptions.preferredBranch) {
      onChange(branchOptions.preferredBranch);
    }
  }, [branchOptions, onChange, projectName, value]);

  function matchingBranches(input: string) {
    return branchOptions.uniqueBranches
      .filter((branch) => branch.toLowerCase().includes(input.toLowerCase()))
      .slice(0, 8);
  }

  const filtered = useMemo(() => matchingBranches(query), [branchOptions, query]);

  return (
    <Flex direction="column" gap="1" style={{ position: "relative" }}>
      <Text size="2" weight="medium">
        Base branch
      </Text>
      <TextField.Root
        aria-label="Base branch"
        role="combobox"
        aria-expanded={open}
        aria-controls="new-session-branch-listbox"
        aria-autocomplete="list"
        placeholder={projectName ? "Search branches…" : "Select a project first"}
        value={query}
        onChange={(e) => {
          const nextQuery = e.target.value;
          setQuery(nextQuery);
          onChange(branchOptions.uniqueBranches.includes(nextQuery) ? nextQuery : "");
          setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        onKeyDown={(event) => {
          const currentQuery =
            event.target instanceof HTMLInputElement && event.target.value
              ? event.target.value
              : latestQuery.current;
          const currentMatches = matchingBranches(currentQuery);
          if (event.key === "Enter" && currentMatches.length > 0) {
            event.preventDefault();
            onChange(currentMatches[0]);
            setQuery(currentMatches[0]);
            setOpen(false);
          }
          if (event.key === "Escape") {
            setOpen(false);
            setQuery(latestValue.current);
          }
        }}
        onBlur={() =>
          setTimeout(() => {
            setOpen(false);
            if (!latestValue.current && latestQuery.current) {
              const nextBranch = matchingBranches(latestQuery.current)[0];
              if (nextBranch) {
                onChange(nextBranch);
                setQuery(nextBranch);
              }
            }
          }, 150)
        }
        disabled={!projectName}
      >
        <TextField.Slot
          side="right"
          style={{ cursor: "pointer", pointerEvents: "auto" }}
          onClick={() => setOpen((current) => !current)}
        >
          <CaretDown size={14} />
        </TextField.Slot>
      </TextField.Root>
      {open && (
        <Box className={branchComboboxList} id="new-session-branch-listbox" role="listbox">
          {filtered.length > 0 ? (
            filtered.map((branch) => (
              <Box
                key={branch}
                className={branchComboboxItem}
                role="option"
                aria-selected={branch === value}
                data-selected={branch === value ? "true" : "false"}
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
            ))
          ) : (
            <Box px="3" py="2">
              <Text size="2" color="gray">
                No matching branches
              </Text>
            </Box>
          )}
        </Box>
      )}
    </Flex>
  );
}

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

// r[ui.session-list.create]
// r[proto.create-session]
// r[session.create]
export function NewSessionDialog({
  open,
  onOpenChange,
  preselectedProject,
  preselectedCaptainKind,
  preselectedMateKind,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  preselectedProject?: string;
  preselectedCaptainKind?: AgentKind;
  preselectedMateKind?: AgentKind;
}) {
  const navigate = useNavigate();
  const projects = useProjects().filter((p) => p.valid);
  const discovery = useAgentDiscovery();

  const defaultProject = preselectedProject ?? (projects.length === 1 ? projects[0].name : "");
  const [projectName, setProjectName] = useState(defaultProject);
  const [captainKind, setCaptainKind] = useState<AgentKind>(
    preselectedCaptainKind ?? { tag: "Claude" },
  );
  const [mateKind, setMateKind] = useState<AgentKind>(preselectedMateKind ?? { tag: "Claude" });
  const [branch, setBranch] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }

    setProjectName(defaultProject);
    setBranch("");
    setCreateError(null);
    if (preselectedCaptainKind) setCaptainKind(preselectedCaptainKind);
    if (preselectedMateKind) setMateKind(preselectedMateKind);
  }, [defaultProject, open]);

  useEffect(() => {
    const fallbackKind = firstAvailableAgentKind(discovery);
    if (!fallbackKind) {
      return;
    }

    if (!isAgentKindAvailable(captainKind, discovery)) {
      setCaptainKind(fallbackKind);
    }
    if (!isAgentKindAvailable(mateKind, discovery)) {
      setMateKind(fallbackKind);
    }
  }, [captainKind, mateKind, discovery]);

  async function handleCreate() {
    if (!projectName || !branch) return;
    setCreateError(null);
    setSubmitting(true);
    try {
      const client = await getShipClient();
      const result = await client.createSession({
        project: projectName,
        captain_kind: captainKind,
        mate_kind: mateKind,
        base_branch: branch,
        mcp_servers: null,
      });
      if (result.tag === "Failed") {
        setCreateError(result.message);
        return;
      }
      await refreshSessionList();
      onOpenChange(false);
      navigate(`/sessions/${result.slug}`);
    } finally {
      setSubmitting(false);
    }
  }

  const createDisabled =
    !projectName ||
    !branch ||
    submitting ||
    !isAgentKindAvailable(captainKind, discovery) ||
    !isAgentKindAvailable(mateKind, discovery);

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Content key={String(open)} maxWidth="480px" style={{ overflow: "visible" }}>
        <Dialog.Title>New Session</Dialog.Title>
        <Dialog.Description size="2" color="gray">
          Configure a new agent session with a project, agents, and branch.
        </Dialog.Description>
        {createError ? (
          <Callout.Root color="red" mt="3">
            <Callout.Text>{createError}</Callout.Text>
          </Callout.Root>
        ) : null}
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
            opencodeAvailable={discovery.opencode}
          />

          <AgentKindControl
            label="Mate"
            value={mateKind}
            onChange={setMateKind}
            claudeAvailable={discovery.claude}
            codexAvailable={discovery.codex}
            opencodeAvailable={discovery.opencode}
          />

          <BranchCombobox projectName={projectName} value={branch} onChange={setBranch} />

          <Flex gap="2" justify="end" mt="1">
            <Dialog.Close>
              <Button variant="soft" color="gray">
                Cancel
              </Button>
            </Dialog.Close>
            <Button disabled={createDisabled} loading={submitting} onClick={handleCreate}>
              Create session
            </Button>
          </Flex>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}

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

const LAST_PROJECT_KEY = "ship.lastProject";

function relativeTime(dateString: string): string {
  const diff = (Date.now() - new Date(dateString).getTime()) / 1000;
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 518400) return `${Math.floor(diff / 86400)}d ago`;
  return new Date(dateString).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

// r[ui.session-list.layout]
function SessionCard({
  session,
  archivingId,
  onArchive,
}: {
  session: SessionSummary;
  archivingId: string | null;
  onArchive: (session: SessionSummary, force: boolean) => void;
}) {
  const diffStats = useWorktreeDiffStats(session.id);
  const hasActivity =
    diffStats !== null &&
    Number(diffStats.lines_added) +
      Number(diffStats.lines_removed) +
      Number(diffStats.files_changed) >
      0;

  return (
    <Card className={sessionCard}>
      <Flex direction="column" gap="2">
        {/* Row 1: title + status badge */}
        <Flex align="center" gap="2">
          {session.title ? (
            <Text size="3" weight="bold" style={{ lineHeight: 1.4 }}>
              {session.title}
            </Text>
          ) : session.startup_state.tag !== "Ready" ? (
            <Text size="2" color="gray">
              {session.startup_state.tag === "Pending"
                ? "Session startup is queued."
                : session.startup_state.message}
            </Text>
          ) : null}
          {session.task_status && (
            <Badge color={STATUS_COLOR[session.task_status.tag]} size="1" ml="auto">
              {session.task_status.tag}
            </Badge>
          )}
        </Flex>

        {/* Row 2: current task title */}
        {session.current_task_title && (
          <Text size="2" color="gray" style={{ lineHeight: 1.4 }}>
            {session.current_task_title}
          </Text>
        )}

        {/* Row 3: metadata footer */}
        <Flex align="center" gap="2" wrap="wrap">
          <Badge color="gray" variant="outline" size="1">
            {session.project}
          </Badge>
          <Code variant="ghost" size="1">
            {session.branch_name}
          </Code>
          <Text size="1" color="gray">
            ·
          </Text>
          <AgentKindLabel kind={session.captain.kind} />
          <AgentKindLabel kind={session.mate.kind} />
          {hasActivity && diffStats && (
            <>
              <Text size="1" color="gray">
                ·
              </Text>
              <Text size="1" style={{ color: "var(--green-11)" }}>
                +{String(diffStats.lines_added)}
              </Text>
              <Text size="1" style={{ color: "var(--red-11)" }}>
                −{String(diffStats.lines_removed)}
              </Text>
              <Text size="1" color="gray">
                · {String(diffStats.files_changed)} files
              </Text>
            </>
          )}
          <Text size="1" color="gray" ml="auto">
            {relativeTime(session.created_at)}
          </Text>
          {/* r[proto.archive-session] */}
          <Tooltip content="Archive session">
            <IconButton
              size="1"
              variant="ghost"
              color="gray"
              loading={archivingId === session.id}
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                onArchive(session, false);
              }}
            >
              <Archive size={14} />
            </IconButton>
          </Tooltip>
        </Flex>
      </Flex>
    </Card>
  );
}

// r[view.session-list]
// r[ui.session-list.layout]
// r[session.list]
export function SessionListPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const rawFilter = searchParams.get("project");
  const projectFilter = rawFilter ?? undefined;

  useEffect(() => {
    if (rawFilter) {
      localStorage.setItem(LAST_PROJECT_KEY, rawFilter);
    } else {
      const last = localStorage.getItem(LAST_PROJECT_KEY);
      if (last) {
        setSearchParams({ project: last }, { replace: true });
      }
    }
  }, [rawFilter, setSearchParams]);

  const allProjects = useProjects();
  const validProjects = allProjects.filter((p) => p.valid);
  const sessions = useSessionList(projectFilter);
  const sortedSessions = useMemo(() => sortSessions(sessions), [sessions]);

  const [newSessionOpen, setNewSessionOpen] = useState(false);
  const [addProjectOpen, setAddProjectOpen] = useState(false);

  const [archivingId, setArchivingId] = useState<string | null>(null);
  const [archiveConfirm, setArchiveConfirm] = useState<{
    session: SessionSummary;
    unmergedCommits: string[];
  } | null>(null);
  const [archiveError, setArchiveError] = useState<string | null>(null);

  async function handleArchive(session: SessionSummary, force: boolean) {
    setArchivingId(session.id);
    setArchiveError(null);
    try {
      const client = await getShipClient();
      const result = await client.archiveSession({ id: session.id, force });
      if (result.tag === "Archived") {
        setArchiveConfirm(null);
        await refreshSessionList();
      } else if (result.tag === "RequiresConfirmation") {
        setArchiveConfirm({ session, unmergedCommits: result.unmerged_commits });
      } else if (result.tag === "Failed") {
        setArchiveError(result.message);
      }
    } catch (e) {
      setArchiveError(e instanceof Error ? e.message : String(e));
    } finally {
      setArchivingId(null);
    }
  }

  const noProjects = validProjects.length === 0;

  return (
    <Box p="4" style={{ maxWidth: 720, margin: "0 auto" }}>
      {/* r[ui.session-list.project-filter] */}
      {!noProjects && (
        <Flex mb="4" align="center" gap="2" justify="between" wrap="wrap">
          <Select.Root
            value={projectFilter ?? "__all__"}
            onValueChange={(v) => {
              if (v === "__add_project__") {
                setAddProjectOpen(true);
                return;
              }
              if (v === "__all__") {
                localStorage.removeItem(LAST_PROJECT_KEY);
                setSearchParams({});
              } else {
                setSearchParams({ project: v });
              }
            }}
          >
            <Select.Trigger aria-label="Filter projects" placeholder="All projects" />
            <Select.Content>
              <Select.Item value="__all__">All projects</Select.Item>
              {validProjects.map((p) => (
                <Select.Item key={p.name} value={p.name}>
                  {p.name}
                </Select.Item>
              ))}
              <Select.Separator />
              <Select.Item value="__add_project__">Add Project</Select.Item>
            </Select.Content>
          </Select.Root>
          <Button size="2" onClick={() => setNewSessionOpen(true)}>
            <Plus size={16} />
            New Session
          </Button>
        </Flex>
      )}
      {allProjects.some((p) => !p.valid) && (
        <Callout.Root color="amber" size="1" mb="4">
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

      {/* r[ui.session-list.empty] */}
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
              {projectFilter ? `No sessions in ${projectFilter} yet.` : "No sessions yet."}
            </Callout.Text>
          </Callout.Root>
        </Flex>
      ) : (
        <>
          {archiveError && (
            <Callout.Root color="red" size="1" mb="3">
              <Callout.Icon>
                <WarningCircle size={16} />
              </Callout.Icon>
              <Callout.Text>{archiveError}</Callout.Text>
            </Callout.Root>
          )}

          <Flex direction="column" gap="3">
            {sessions.map((session) => (
              <Link
                key={session.id}
                to={`/sessions/${session.slug}`}
                style={{ textDecoration: "none", color: "inherit" }}
              >
                <SessionCard
                  session={session}
                  archivingId={archivingId}
                  onArchive={handleArchive}
                />
              </Link>
            ))}
          </Flex>
        </>
      )}

      <NewSessionDialog
        open={newSessionOpen}
        onOpenChange={setNewSessionOpen}
        preselectedProject={projectFilter}
      />
      <AddProjectDialog open={addProjectOpen} onOpenChange={setAddProjectOpen} />
      {archiveConfirm && (
        <ArchiveSessionDialog
          session={archiveConfirm.session}
          unmergedCommits={archiveConfirm.unmergedCommits}
          onConfirm={() => handleArchive(archiveConfirm.session, true)}
          onCancel={() => setArchiveConfirm(null)}
          archiving={archivingId === archiveConfirm.session.id}
        />
      )}
    </Box>
  );
}
