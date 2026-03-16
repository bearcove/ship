use facet_mcp::CallToolResult;
use serde_json::{Value, json};

pub async fn kagi_web_search(
    http_client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> CallToolResult {
    let response = match http_client
        .post("https://kagi.com/api/v0/fastgpt")
        .header("Authorization", format!("Bot {api_key}"))
        .json(&json!({ "query": query }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return CallToolResult::error(format!("web_search request failed: {e}")),
    };

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return CallToolResult::error(format!(
            "web_search request failed with status {status}: {body}"
        ));
    }

    let body: Value = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            return CallToolResult::error(format!("failed to parse web_search response: {e}"));
        }
    };

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

    CallToolResult::text(text)
}
