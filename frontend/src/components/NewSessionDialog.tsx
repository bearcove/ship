import { useState, useMemo, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import {
  Box,
  Button,
  Callout,
  Dialog,
  Flex,
  Select,
  Text,
  TextField,
} from "@radix-ui/themes";
import { CaretDown } from "@phosphor-icons/react";
import { useProjects } from "../hooks/useProjects";
import { refreshSessionList } from "../hooks/useSessionList";
import { useAgentDiscovery } from "../hooks/useAgentDiscovery";
import { useBranches } from "../hooks/useBranches";
import {
  branchComboboxItem,
  branchComboboxList,
} from "../styles/session-list.css";
import type { AgentKind } from "../generated/ship";
import { getShipClient } from "../api/client";
import { useAgentPresets } from "../hooks/useAgentPresets";
import { UnifiedAgentPicker } from "./UnifiedAgentPicker";

// r[session.agent.kind]
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
  const { presets, loading: presetsLoading } = useAgentPresets();

  const defaultProject = preselectedProject ?? (projects.length === 1 ? projects[0].name : "");
  const [projectName, setProjectName] = useState(defaultProject);
  const [captainKind, setCaptainKind] = useState<AgentKind>(
    preselectedCaptainKind ?? { tag: "Claude" },
  );
  const [mateKind, setMateKind] = useState<AgentKind>(preselectedMateKind ?? { tag: "Claude" });
  const [captainPresetId, setCaptainPresetId] = useState<string | null>(null);
  const [matePresetId, setMatePresetId] = useState<string | null>(null);
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
    setCaptainPresetId(null);
    setMatePresetId(null);
    if (preselectedCaptainKind) setCaptainKind(preselectedCaptainKind);
    if (preselectedMateKind) setMateKind(preselectedMateKind);
  }, [defaultProject, open]);

  useEffect(() => {
    if (!open) return;
    getShipClient().then((client) => {
      client.getNewSessionDefaults().then((defaults) => {
        if (!defaults) return;
        if (!preselectedProject && defaults.project) {
          setProjectName(defaults.project);
        }
        if (!preselectedCaptainKind && defaults.captain_preset_id) {
          setCaptainPresetId(defaults.captain_preset_id);
        }
        if (!preselectedMateKind && defaults.mate_preset_id) {
          setMatePresetId(defaults.mate_preset_id);
        }
      });
    });
  }, [open]);

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

  const selectedCaptainPreset = captainPresetId
    ? presets.find((preset) => preset.id === captainPresetId) ?? null
    : null;
  const selectedMatePreset = matePresetId
    ? presets.find((preset) => preset.id === matePresetId) ?? null
    : null;
  const effectiveCaptainKind = selectedCaptainPreset?.kind ?? captainKind;
  const effectiveMateKind = selectedMatePreset?.kind ?? mateKind;

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
        captain_preset_id: captainPresetId,
        mate_preset_id: matePresetId,
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
    !isAgentKindAvailable(effectiveCaptainKind, discovery) ||
    !isAgentKindAvailable(effectiveMateKind, discovery);

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Content
        key={String(open)}
        maxWidth="480px"
        style={{ overflow: "visible" }}
        onOpenAutoFocus={(e) => e.preventDefault()}
        onKeyDown={(e) => {
          if (e.key !== "Enter") return;
          const tag = (e.target as HTMLElement).tagName;
          if (tag === "BUTTON" || tag === "INPUT" || tag === "TEXTAREA") return;
          if ((e.target as HTMLElement).closest('[role="listbox"], [role="option"], [role="combobox"]')) return;
          if (createDisabled) return;
          e.preventDefault();
          void handleCreate();
        }}
      >
        <Dialog.Title>New Session</Dialog.Title>
        <Dialog.Description size="2" color="gray">
          Configure a new agent session with a project and branch.
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

          <UnifiedAgentPicker
            label="Captain"
            selectedPresetId={captainPresetId}
            presets={presets}
            onSelectPreset={(preset) => {
              setCaptainPresetId(preset.id);
              setCaptainKind(preset.kind);
            }}
          />

          <UnifiedAgentPicker
            label="Mate"
            selectedPresetId={matePresetId}
            presets={presets}
            onSelectPreset={(preset) => {
              setMatePresetId(preset.id);
              setMateKind(preset.kind);
            }}
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
