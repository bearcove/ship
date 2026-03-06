import type {
  ContentBlock,
  JsonValue,
  PermissionOption,
  PermissionOptionKind,
  ToolCallContent,
  ToolCallKind,
  ToolCallLocation,
  ToolTarget,
} from "../../generated/ship";
import { formatDisplayPath, formatDisplayText } from "../../utils/displayPath";

function toPlainJson(value: JsonValue): unknown {
  switch (value.tag) {
    case "Null":
      return null;
    case "Bool":
    case "Number":
    case "String":
      return value.value;
    case "Array":
      return value.items.map(toPlainJson);
    case "Object":
      return Object.fromEntries(
        value.entries.map((entry) => [entry.key, toPlainJson(entry.value)]),
      );
  }
}

export function jsonValueToString(value: JsonValue | null | undefined): string {
  if (!value) return "";
  return formatDisplayText(JSON.stringify(toPlainJson(value), null, 2));
}

export function displayTargetPath(path: string, displayPath: string | null): string {
  return displayPath ?? formatDisplayPath(path);
}

function changedLineCounts(oldText: string, newText: string): { added: number; removed: number } {
  const oldLines = oldText.split("\n");
  const newLines = newText.split("\n");

  let prefix = 0;
  while (
    prefix < oldLines.length &&
    prefix < newLines.length &&
    oldLines[prefix] === newLines[prefix]
  ) {
    prefix += 1;
  }

  let oldSuffix = oldLines.length - 1;
  let newSuffix = newLines.length - 1;
  while (
    oldSuffix >= prefix &&
    newSuffix >= prefix &&
    oldLines[oldSuffix] === newLines[newSuffix]
  ) {
    oldSuffix -= 1;
    newSuffix -= 1;
  }

  return {
    added: Math.max(0, newSuffix - prefix + 1),
    removed: Math.max(0, oldSuffix - prefix + 1),
  };
}

export function diffStats(contents: ToolCallContent[]): string {
  const diff = contents.find((item) => item.tag === "Diff");
  if (!diff) return "";
  const { added, removed } = changedLineCounts(diff.old_text ?? "", diff.new_text);
  return `+${added} -${removed}`;
}

export function summarizeTarget(
  target: ToolTarget | null,
  kind: ToolCallKind | null,
  contents: ToolCallContent[],
): string {
  if (!target) return "";

  switch (target.tag) {
    case "None":
      return "";
    case "File": {
      const path = displayTargetPath(target.path, target.display_path);
      const stats = kind?.tag === "Edit" || kind?.tag === "Delete" ? diffStats(contents) : "";
      if (stats) return `${path}  ${stats}`;
      return target.line ? `${path}:${target.line}` : path;
    }
    case "Move":
      return `${displayTargetPath(target.source_path, target.source_display_path)} -> ${displayTargetPath(target.destination_path, target.destination_display_path)}`;
    case "Search":
      return target.query ?? target.glob ?? target.display_path ?? target.path ?? "";
    case "Command":
      return target.command;
  }
}

export function optionTone(kind: PermissionOptionKind): {
  color?: "gray";
  variant: "solid" | "soft" | "outline";
} {
  switch (kind.tag) {
    case "AllowOnce":
      return { variant: "solid" };
    case "AllowAlways":
      return { variant: "outline" };
    case "RejectOnce":
    case "RejectAlways":
      return { color: "gray", variant: "soft" };
    case "Other":
      return { color: "gray", variant: "outline" };
  }
}

export function permissionOptionLabel(option: PermissionOption, _toolName: string): string {
  switch (option.kind.tag) {
    case "AllowOnce":
      return "Approve";
    case "AllowAlways":
      return "Always";
    case "RejectOnce":
    case "RejectAlways":
      return "Deny";
    case "Other":
      return option.label;
  }
}

export function permissionOptionTooltip(option: PermissionOption): string | undefined {
  if (option.kind.tag === "AllowAlways") {
    return "Applies for the remainder of the current task.";
  }
  return undefined;
}

export function firstAllowOption(options: PermissionOption[] | null): PermissionOption | null {
  return (
    options?.find(
      (option) => option.kind.tag === "AllowOnce" || option.kind.tag === "AllowAlways",
    ) ?? null
  );
}

export function firstRejectOption(options: PermissionOption[] | null): PermissionOption | null {
  return (
    options?.find(
      (option) => option.kind.tag === "RejectOnce" || option.kind.tag === "RejectAlways",
    ) ?? null
  );
}

type ToolCallBlockType = Extract<ContentBlock, { tag: "ToolCall" }>;

function parseLegacyArgs(raw: string): Record<string, string> {
  try {
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") {
      return Object.fromEntries(
        Object.entries(parsed).map(([key, value]) => [
          key,
          typeof value === "string"
            ? formatDisplayText(value)
            : formatDisplayText(JSON.stringify(value, null, 2)),
        ]),
      );
    }
  } catch {
    // ignored
  }
  return {};
}

function firstLegacyPath(locations: ToolCallLocation[], args: Record<string, string>): string {
  const path =
    args.path ?? args.file_path ?? locations[0]?.display_path ?? locations[0]?.path ?? "";
  return path ? formatDisplayPath(path) : "";
}

function legacyCollapsedSummary(
  toolName: string,
  args: Record<string, string>,
  contents: ToolCallContent[],
  locations: ToolCallLocation[],
): string {
  const name = toolName.toLowerCase();
  if (["read", "read file", "read_file", "readtextfile"].includes(name)) {
    return firstLegacyPath(locations, args);
  }
  if (["write", "write file", "write_file", "edit", "notebookedit"].includes(name)) {
    const path = firstLegacyPath(locations, args);
    const stats = diffStats(contents);
    return stats ? `${path}  ${stats}` : path;
  }
  if (["bash", "terminal", "run", "create terminal", "create_terminal"].includes(name)) {
    return args.command ?? args.cmd ?? "";
  }
  if (["grep", "glob", "search"].includes(name)) {
    return args.pattern ?? args.query ?? args.glob ?? args.include ?? "";
  }
  return firstLegacyPath(locations, args) || args.command || args.pattern || "";
}

export function collapsedSummary(block: ToolCallBlockType): string {
  const summary = summarizeTarget(block.target, block.kind, block.content);
  if (summary) return summary;
  return legacyCollapsedSummary(
    block.tool_name,
    parseLegacyArgs(block.arguments),
    block.content,
    block.locations,
  );
}
