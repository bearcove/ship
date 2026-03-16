use facet::Facet;
use facet_mcp::ToolError;

/// Wrapper for the Kagi API key, stored in ToolCtx.
pub struct KagiApiKey(pub String);

#[derive(Debug, Facet)]
pub struct WebSearchArgs {
    /// The search query.
    pub query: String,
}

#[derive(Debug, Facet)]
pub struct WebSearchResult {
    /// The search result text with references.
    pub text: String,
}

facet_mcp::tool! {
    /// Search the web using Kagi FastGPT. Returns an AI-synthesized answer and a list of references.
    pub(crate) async fn web_search(args: WebSearchArgs, ctx: &ToolCtx) -> WebSearchResult {
        let Some(api_key) = ctx.try_get::<KagiApiKey>() else {
            return Err(ToolError::new("KAGI_API_KEY is not configured"));
        };
        let http = ctx.get::<reqwest::Client>();
        let text = crate::kagi::kagi_web_search(http, &api_key.0, &args.query).await?;
        Ok(WebSearchResult { text })
    }
}
