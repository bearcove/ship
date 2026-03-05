import { Box, Code } from "@radix-ui/themes";
import ReactMarkdown from "react-markdown";
import type { TextBlock as TextBlockType } from "../../types";
import { toolCallBody } from "../../styles/session-view.css";

interface Props {
  block: TextBlockType;
}

export function TextBlock({ block }: Props) {
  return (
    <Box>
      <ReactMarkdown
        components={{
          code({ children, className }: { children?: React.ReactNode; className?: string }) {
            const isBlock = className?.startsWith("language-");
            if (isBlock) {
              return (
                <Box
                  className={toolCallBody}
                  style={{ background: "var(--gray-a2)", borderRadius: "var(--radius-2)" }}
                >
                  <code>{children}</code>
                </Box>
              );
            }
            return <Code size="1">{children}</Code>;
          },
          p({ children }: { children?: React.ReactNode }) {
            return (
              <p style={{ margin: "0 0 var(--space-2)", fontSize: "var(--font-size-2)" }}>
                {children}
              </p>
            );
          },
        }}
      >
        {block.content}
      </ReactMarkdown>
    </Box>
  );
}
