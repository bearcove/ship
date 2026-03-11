import { useEffect, useState } from "react";
import { Popover, IconButton, Text, Flex } from "@radix-ui/themes";
import { QrCode } from "@phosphor-icons/react";
import { QRCodeSVG } from "qrcode.react";
import { getShipClient } from "../api/client";

export function QrCodeButton() {
  const [open, setOpen] = useState(false);
  const [url, setUrl] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!open) return;

    let active = true;
    setLoading(true);

    async function fetchInfo() {
      const client = await getShipClient();
      const info = await client.getServerInfo();
      if (!active) return;
      const nonLoopback = info.http_urls.find((u) => {
        try {
          const host = new URL(u).hostname;
          return host !== "localhost" && host !== "127.0.0.1" && host !== "::1";
        } catch {
          return false;
        }
      });
      setUrl(nonLoopback ?? info.http_urls[0] ?? null);
      setLoading(false);
    }

    fetchInfo();

    return () => {
      active = false;
    };
  }, [open]);

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger>
        <IconButton variant="ghost" size="2" aria-label="Show QR code for LAN access">
          <QrCode size={18} />
        </IconButton>
      </Popover.Trigger>
      <Popover.Content align="end" style={{ padding: "16px" }}>
        {loading ? (
          <Text size="2" color="gray">
            Loading…
          </Text>
        ) : url ? (
          <Flex direction="column" align="center" gap="2">
            <QRCodeSVG value={url} size={200} />
            <Text size="1" color="gray" style={{ wordBreak: "break-all", maxWidth: "200px" }}>
              {url}
            </Text>
          </Flex>
        ) : (
          <Text size="2" color="gray">
            No LAN address available
          </Text>
        )}
      </Popover.Content>
    </Popover.Root>
  );
}
