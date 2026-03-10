import { decodeWithSchema, encodeWithSchema } from "@bearcove/roam-postcard";
import { describe, expect, it } from "vitest";
import type { SubscribeMessage } from "./ship";
import { ship_descriptor } from "./ship";

function subscribeMessageSchema() {
  const registry = ship_descriptor.schema_registry;
  if (!registry) {
    throw new Error("ship descriptor schema registry is missing");
  }
  const schema = registry.get("SubscribeMessage");
  if (!schema) {
    throw new Error("SubscribeMessage schema is missing");
  }
  return schema;
}

describe("generated ship schema", () => {
  // r[verify event.subscribe]
  // r[verify acp.content-blocks]
  it("round-trips live BlockAppend tool call events through the generated subscribe schema", () => {
    const schema = subscribeMessageSchema();
    const message: SubscribeMessage = {
      tag: "Event",
      value: {
        seq: 40n,
        timestamp: "2026-01-01T00:00:00Z",
        event: {
          tag: "BlockAppend",
          block_id: "block-40",
          role: { tag: "Captain" },
          block: {
            tag: "ToolCall",
            tool_call_id: "toolu_1",
            tool_name: "ship_steer",
            arguments: '{"message":"hello"}',
            kind: { tag: "Other" },
            target: { tag: "None" },
            raw_input: {
              tag: "Object",
              entries: [{ key: "message", value: { tag: "String", value: "hello" } }],
            },
            raw_output: {
              tag: "Object",
              entries: [{ key: "queued", value: { tag: "Bool", value: true } }],
            },
            locations: [],
            status: { tag: "Success" },
            content: [{ tag: "Text", text: "Queued steer for human review." }],
            error: null,
          },
        },
      },
    };

    const encoded = encodeWithSchema(message, schema, ship_descriptor.schema_registry);
    const decoded = decodeWithSchema(encoded, 0, schema, ship_descriptor.schema_registry).value;

    expect(decoded).toEqual(message);
  });
});
