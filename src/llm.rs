//! Optional Claude-backed generation. A thin async client over the Anthropic
//! Messages API using the existing `reqwest` dependency. Activates only when
//! `ANTHROPIC_API_KEY` is set; otherwise callers surface a graceful note so the
//! server stays fully usable offline.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

/// Default model — cheapest current option, good for bulk document work.
pub const DEFAULT_MODEL: &str = "claude-haiku-4-5-20251001";

/// Whether an API key is configured (tools use this to degrade gracefully).
pub fn api_key() -> Option<String> {
    std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.trim().is_empty())
}

/// The standard "no key configured" note returned by ai_* tools.
pub fn no_key_note(tool: &str) -> String {
    format!(
        "> **`{}` requires an Anthropic API key.**\n>\n> Set `ANTHROPIC_API_KEY` in the server \
         environment to enable Claude-backed generation. Optional: `ANTHROPIC_MODEL` (default \
         `{}`) and `ANTHROPIC_BASE_URL`.\n>\n> Local alternatives that need no key: \
         `summarize_document`, `extract_keywords`, `classify_document`.",
        tool, DEFAULT_MODEL
    )
}

/// Resolve the model to use: explicit arg > `ANTHROPIC_MODEL` env > default.
pub fn resolve_model(explicit: Option<&str>) -> String {
    explicit
        .map(|s| s.to_string())
        .or_else(|| std::env::var("ANTHROPIC_MODEL").ok().filter(|m| !m.trim().is_empty()))
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

/// Send a single-turn prompt to the Anthropic Messages API and return the text.
/// `system` is optional; `max_tokens` caps the response length.
pub async fn complete(
    prompt: &str,
    system: Option<&str>,
    model: &str,
    max_tokens: u32,
) -> Result<String> {
    let key = api_key().ok_or_else(|| anyhow!("ANTHROPIC_API_KEY not set"))?;
    let base = std::env::var("ANTHROPIC_BASE_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
    let url = format!("{}/v1/messages", base.trim_end_matches('/'));

    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": [{ "role": "user", "content": prompt }],
    });
    if let Some(sys) = system {
        body["system"] = json!(sys);
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(serde_json::to_string(&body)?)
        .send()
        .await
        .map_err(|e| anyhow!("Request to Anthropic API failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!("Anthropic API error {}: {}", status, text));
    }

    let parsed: Value = serde_json::from_str(&text)
        .map_err(|e| anyhow!("Invalid API response: {}", e))?;
    // content is an array of blocks; concatenate all text blocks.
    let out = parsed
        .get("content")
        .and_then(|c| c.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();

    if out.is_empty() {
        return Err(anyhow!("Empty response from Anthropic API: {}", text));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_model_default() {
        // With no explicit arg and (assuming) no env, falls back to default.
        std::env::remove_var("ANTHROPIC_MODEL");
        assert_eq!(resolve_model(None), DEFAULT_MODEL);
        assert_eq!(resolve_model(Some("claude-opus-4-8")), "claude-opus-4-8");
    }

    #[test]
    fn test_no_key_note_mentions_env() {
        let note = no_key_note("ai_summarize");
        assert!(note.contains("ANTHROPIC_API_KEY"));
        assert!(note.contains("ai_summarize"));
    }
}
