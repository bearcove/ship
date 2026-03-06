import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { renderWithTheme } from "../../test/render";
import { PermissionBlock } from "./PermissionBlock";

const block = {
  tag: "Permission" as const,
  permission_id: "perm-1",
  tool_call_id: "toolu_1",
  tool_name: "Write File",
  description: "Write file",
  arguments: '{"path":"src/lib.rs"}',
  kind: { tag: "Edit" as const },
  target: {
    tag: "File" as const,
    path: "/repo/src/lib.rs",
    display_path: "src/lib.rs",
    line: null,
  },
  raw_input: null,
  options: [
    {
      option_id: "allow-once",
      label: "Allow once",
      kind: { tag: "AllowOnce" as const },
    },
    {
      option_id: "allow-always",
      label: "Allow always",
      kind: { tag: "AllowAlways" as const },
    },
    {
      option_id: "reject-once",
      label: "Reject once",
      kind: { tag: "RejectOnce" as const },
    },
  ],
  resolution: null,
};

describe("PermissionBlock", () => {
  // r[verify acp.permissions]
  it("renders typed permission options and resolves the selected option id", async () => {
    const onResolve = vi.fn(async () => undefined);
    renderWithTheme(<PermissionBlock block={block} onResolve={onResolve} />);

    expect(screen.getByText("Permission request")).toBeInTheDocument();
    expect(screen.getByText(/src\/lib\.rs/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Approve" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Approve all Write File" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deny" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Approve all Write File" }));
    expect(onResolve).toHaveBeenCalledWith("allow-always");
  });

  // r[verify view.permission-dialog]
  // r[verify ui.permission.actions]
  it("shortens worktree paths in the details view", async () => {
    renderWithTheme(
      <PermissionBlock
        block={{
          ...block,
          raw_input: {
            tag: "Object",
            entries: [
              {
                key: "path",
                value: {
                  tag: "String",
                  value:
                    "/Users/amos/bearcove/ship-fullstack/.ship/worktrees/fullstack/frontend/src/lib.rs",
                },
              },
            ],
          },
        }}
      />,
    );

    fireEvent.click(screen.getByText("Details"));
    expect(screen.getByText(/"path": "frontend\/src\/lib\.rs"/)).toBeInTheDocument();
  });
});
