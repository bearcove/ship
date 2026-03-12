# `fn response` is duplicated identically in `CaptainMcpSessionService` and `MateMcpSessionService`

In `crates/ship-server/src/ship_impl.rs`, both `impl CaptainMcpSessionService` (line ~5592) and `impl MateMcpSessionService` (line ~5694) define an identical 14-line `fn response`:

```rust
fn response(result: Result<String, String>) -> McpToolCallResponse {
    match result {
        Ok(text) => McpToolCallResponse { text, is_error: false, diffs: vec![] },
        Err(text) => McpToolCallResponse { text, is_error: true, diffs: vec![] },
    }
}
```

Additionally, `McpToolCallResponse { ... }` is constructed inline ~43 times throughout `ship_impl.rs` rather than going through this helper consistently.

## Fix

Make `response` a free function (or a method on `McpToolCallResponse`) and use it consistently throughout. This is a minor cleanup but the duplication is a sign of the broader `ship_impl.rs` monolith problem (see bug 030).
