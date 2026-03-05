import { Callout } from "@radix-ui/themes";
import { WifiSlash } from "@phosphor-icons/react";

interface Props {
  connected: boolean;
}

// r[ui.error.connection]
export function ConnectionBanner({ connected }: Props) {
  if (connected) return null;
  return (
    <Callout.Root
      color="red"
      size="1"
      style={{ borderRadius: 0, borderLeft: "none", borderRight: "none", borderTop: "none" }}
    >
      <Callout.Icon>
        <WifiSlash size={16} />
      </Callout.Icon>
      <Callout.Text>Connection lost — attempting to reconnect…</Callout.Text>
    </Callout.Root>
  );
}
