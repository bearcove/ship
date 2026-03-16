use facet_mcp::ToolError;
use serde_json::{Value, json};

pub async fn kagi_web_search(
    http_client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> Result<String, ToolError> {
    let response = http_client
        .post("https://kagi.com/api/v0/fastgpt")
        .header("Authorization", format!("Bot {api_key}"))
        .json(&json!({ "query": query }))
        .send()
        .await
        .map_err(|e| ToolError::new(format!("web_search request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ToolError::new(format!(
            "web_search request failed with status {status}: {body}"
        )));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|e| ToolError::new(format!("failed to parse web_search response: {e}")))?;

    let output = body
        .get("data")
        .and_then(|d| d.get("output"))
        .and_then(Value::as_str)
        .unwrap_or("");

    let mut text = output.to_owned();

    if let Some(refs) = body
        .get("data")
        .and_then(|d| d.get("references"))
        .and_then(Value::as_array)
        && !refs.is_empty()
    {
        text.push_str("\n\n## References\n");
        for r in refs {
            let title = r.get("title").and_then(Value::as_str).unwrap_or("Untitled");
            let url = r.get("url").and_then(Value::as_str).unwrap_or("");
            let snippet = r.get("snippet").and_then(Value::as_str).unwrap_or("");
            if snippet.is_empty() {
                text.push_str(&format!("- [{title}]({url})\n"));
            } else {
                text.push_str(&format!("- [{title}]({url}): {snippet}\n"));
            }
        }
    }

    Ok(text)
}
