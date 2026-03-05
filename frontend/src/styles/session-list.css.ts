import { keyframes, style } from "@vanilla-extract/css";

const pulse = keyframes({
  "0%, 100%": { opacity: 1 },
  "50%": { opacity: 0.3 },
});

export const sessionCard = style({
  cursor: "pointer",
  transition: "box-shadow 0.15s",
  ":hover": {
    boxShadow: "var(--shadow-3)",
  },
});

export const idleDot = style({
  width: 8,
  height: 8,
  borderRadius: "50%",
  background: "var(--amber-9)",
  animation: `${pulse} 1.5s ease-in-out infinite`,
});
