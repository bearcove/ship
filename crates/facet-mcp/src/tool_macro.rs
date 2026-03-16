/// Define a tool with typed args, result, and handler.
///
/// The tool name is the function name. The description comes from the args type's
/// doc comments (via Facet reflection). Input/output schemas are generated from
/// the Facet shapes of the args and result types.
///
/// ```ignore
/// tool! {
///     async fn web_search(args: WebSearchArgs, ctx: &ToolCtx) -> WebSearchResult {
///         let http = ctx.get::<reqwest::Client>();
///         // ...
///     }
/// }
/// ```
///
/// Expands to a module containing:
/// - `pub async fn call(args, ctx) -> Result` — the handler
/// - `pub fn def() -> ToolInfo` — schema + name + description
/// - `pub async fn dispatch(raw, ctx) -> CallToolResult` — deserialize, call, serialize
#[macro_export]
macro_rules! tool {
    (
        async fn $name:ident($args_name:ident: $args:ty, $ctx_name:ident: &ToolCtx) -> $result:ty
        $body:block
    ) => {
        pub mod $name {
            use super::*;

            pub async fn call($args_name: $args, $ctx_name: &$crate::ToolCtx) -> $result
            $body

            pub fn def() -> $crate::ToolInfo {
                let shape = <$args as facet::Facet>::SHAPE;
                let description = if shape.doc.is_empty() {
                    String::new()
                } else {
                    shape.doc.join("\n").trim().to_owned()
                };
                $crate::ToolInfo {
                    name: stringify!($name).to_owned(),
                    description,
                    input_schema: $crate::schema_for::<$args>(),
                    output_schema: $crate::schema_for::<$result>(),
                }
            }

            pub async fn dispatch(
                raw: &facet_json::RawJson<'static>,
                ctx: &$crate::ToolCtx,
            ) -> $crate::CallToolResult {
                let args: $args = match facet_json::from_str(raw.as_ref()) {
                    Ok(a) => a,
                    Err(e) => return $crate::CallToolResult::error(
                        format!("invalid arguments for {}: {e}", stringify!($name))
                    ),
                };
                let result = call(args, ctx).await;
                match facet_json::to_string(&result) {
                    Ok(json) => $crate::CallToolResult::text(json),
                    Err(e) => $crate::CallToolResult::error(
                        format!("failed to serialize result for {}: {e}", stringify!($name))
                    ),
                }
            }
        }
    };
}
