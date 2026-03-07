import type { ContentBlock, Role, BlockPatch } from "../generated/ship";

export interface BlockEntry {
  blockId: string;
  role: Role;
  block: ContentBlock;
}

// r[event.store.immutable-updates]
// r[event.store.structure]
export interface BlockStore {
  blocks: BlockEntry[];
  index: Map<string, number>;
}

export function createBlockStore(): BlockStore {
  return { blocks: [], index: new Map() };
}

// r[event.store.append]
export function appendBlock(
  store: BlockStore,
  blockId: string,
  role: Role,
  block: ContentBlock,
): BlockStore {
  const entry: BlockEntry = { blockId, role, block };
  const blocks = [...store.blocks, entry];
  const index = new Map(store.index);
  index.set(blockId, blocks.length - 1);
  return { blocks, index };
}

// r[event.store.patch]
export function patchBlock(
  store: BlockStore,
  blockId: string,
  patch: BlockPatch,
): BlockStore | null {
  const pos = store.index.get(blockId);
  if (pos === undefined) return null;

  const entry = store.blocks[pos];
  const patched = applyPatch(entry.block, patch);
  if (patched === null) return null;

  const blocks = [...store.blocks];
  blocks[pos] = { ...entry, block: patched };
  return { blocks, index: store.index };
}

function applyPatch(block: ContentBlock, patch: BlockPatch): ContentBlock | null {
  switch (patch.tag) {
    // r[event.patch.text-append]
    case "TextAppend":
      if (block.tag !== "Text") return null;
      return { ...block, text: block.text + patch.text };
    // r[event.patch.tool-call-update]
    case "ToolCallUpdate":
      if (block.tag !== "ToolCall") return null;
      return {
        ...block,
        tool_name: patch.tool_name ?? block.tool_name,
        kind: patch.kind ?? block.kind,
        target: patch.target ?? block.target,
        raw_input: patch.raw_input ?? block.raw_input,
        raw_output: patch.raw_output ?? block.raw_output,
        status: patch.status,
        locations: patch.locations ?? block.locations,
        content: patch.content ?? block.content,
        error: patch.error ?? block.error,
      };
    // r[event.patch.plan-replace]
    case "PlanReplace":
      if (block.tag !== "PlanUpdate") return null;
      return { ...block, steps: patch.steps };
    // r[event.patch.permission-resolve]
    case "PermissionResolve":
      if (block.tag !== "Permission") return null;
      return { ...block, resolution: patch.resolution };
  }
}
