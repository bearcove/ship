/// Define a tool as a type that implements the `Tool` trait.
///
/// ```ignore
/// tool! {
///     /// Add two numbers together.
///     async fn add(args: AddArgs, ctx: &ToolCtx) -> AddResult {
///         AddResult { sum: args.a + args.b }
///     }
/// }
/// ```
///
/// Expands to:
/// - A zero-sized struct (named after the fn, e.g. `add`)
/// - An impl of `Tool` with `Args = AddArgs`, `Result = AddResult`
/// - The handler as the `call` method
#[macro_export]
macro_rules! tool {
    (
        $(#[doc = $doc:literal])*
        async fn $name:ident($args_name:ident: $args:ty, $ctx_name:ident: &ToolCtx) -> $result:ty
        $body:block
    ) => {
        #[allow(non_camel_case_types)]
        struct $name;

        impl $crate::Tool for $name {
            type Args = $args;
            type Result = $result;

            fn name() -> &'static str {
                stringify!($name)
            }

            fn description() -> &'static str {
                concat!($($doc, "\n",)*)
            }

            async fn call($args_name: $args, $ctx_name: &$crate::ToolCtx) -> $result
            $body
        }
    };
}
