import { BracketsAngle, Sparkle } from "@phosphor-icons/react";
import { Badge } from "@radix-ui/themes";
import type { AgentKind } from "../generated/ship";

interface Props {
  kind: AgentKind;
}

// r[frontend.icons]
export function AgentKindIcon({ kind }: Props) {
  const Icon = kind.tag === "Claude" ? Sparkle : BracketsAngle;
  const color = kind.tag === "Claude" ? "violet" : "cyan";

  return (
    <Badge
      color={color}
      variant="soft"
      size="1"
      aria-label={kind.tag}
      title={kind.tag}
      style={{ minWidth: 24, justifyContent: "center" }}
    >
      <Icon size={12} weight="bold" />
    </Badge>
  );
}
