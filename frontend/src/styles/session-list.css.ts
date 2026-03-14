import { style } from "@vanilla-extract/css";

export const branchComboboxList = style({
  position: "absolute",
  top: "100%",
  left: 0,
  right: 0,
  zIndex: 50,
  marginTop: 2,
  overflow: "hidden",
  border: "1px solid var(--gray-a6)",
  borderRadius: "var(--radius-3)",
  background: "var(--color-panel-solid)",
  boxShadow: "var(--shadow-4)",
});

export const branchComboboxItem = style({
  padding: "var(--space-2) var(--space-3)",
  cursor: "pointer",
  selectors: {
    '&[data-selected="true"]': {
      background: "var(--accent-a3)",
    },
    "&:hover": {
      background: "var(--gray-a3)",
    },
  },
});
