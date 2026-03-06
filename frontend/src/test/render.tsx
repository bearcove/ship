import type { ReactElement, ReactNode } from "react";
import { render } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";

function TestTheme({ children }: { children: ReactNode }) {
  return (
    <Theme appearance="dark" accentColor="iris" grayColor="slate" radius="medium" scaling="100%">
      {children}
    </Theme>
  );
}

export function renderWithTheme(ui: ReactElement) {
  return render(ui, { wrapper: TestTheme });
}
