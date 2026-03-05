import { useEffect, useState } from "react";
import { Button, Callout, Flex, Text } from "@radix-ui/themes";
import { Bell } from "@phosphor-icons/react";

const STORAGE_KEY = "ship:notification-asked";

// r[ui.notify.desktop-prompt]
export function NotificationPrompt() {
  const [show, setShow] = useState(false);

  useEffect(() => {
    if (!("Notification" in window)) return;
    if (localStorage.getItem(STORAGE_KEY)) return;
    if (Notification.permission !== "default") {
      localStorage.setItem(STORAGE_KEY, "1");
      return;
    }
    setShow(true);
  }, []);

  if (!show) return null;

  function dismiss() {
    localStorage.setItem(STORAGE_KEY, "1");
    setShow(false);
  }

  function allow() {
    void Notification.requestPermission().finally(() => {
      localStorage.setItem(STORAGE_KEY, "1");
      setShow(false);
    });
  }

  return (
    <Callout.Root color="amber" size="1" style={{ margin: "var(--space-2) var(--space-4)" }}>
      <Callout.Icon>
        <Bell size={16} />
      </Callout.Icon>
      <Callout.Text>
        <Flex align="center" gap="3" wrap="wrap">
          <Text size="2">
            Enable desktop notifications to be alerted when agents need your attention.
          </Text>
          <Flex gap="2">
            <Button size="1" color="amber" onClick={allow}>
              Allow
            </Button>
            <Button size="1" variant="ghost" color="gray" onClick={dismiss}>
              Dismiss
            </Button>
          </Flex>
        </Flex>
      </Callout.Text>
    </Callout.Root>
  );
}
