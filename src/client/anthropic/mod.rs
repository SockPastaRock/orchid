use crate::client::base::BaseClient;
use crate::provider::ProviderError;

use std::env;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MODEL: &str = "claude-3-5-sonnet-20241022";

mod wire;
mod messages;
mod sse;
mod api;

pub use wire::*;
pub use messages::to_wire_message;
pub use sse::SseStream;
pub use api::AnthropicApiClient;

pub struct AnthropicClient {
    base_client: BaseClient,
    api_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
    reasoning_effort: Option<String>,
    extra_headers: Vec<(String, String)>,
}

impl AnthropicClient {
    pub fn new() -> Result<Self, ProviderError> {
        let api_key = env::var("ANTHROPIC_API_KEY").map_err(|_| {
            ProviderError::AuthError("ANTHROPIC_API_KEY environment variable not set".to_string())
        })?;

        let base_client = BaseClient::new()?;

        Ok(AnthropicClient {
            base_client,
            api_url: API_URL.to_string(),
            api_key,
            model: DEFAULT_MODEL.to_string(),
            max_tokens: 8192,
            reasoning_effort: None,
            extra_headers: vec![],
        })
    }

    pub fn from_profile(profile: &crate::config::Profile) -> Result<Self, ProviderError> {
        let raw_key = if profile.api_key.is_empty() {
            env::var("ANTHROPIC_API_KEY").unwrap_or_default()
        } else if let Some(var) = profile.api_key.strip_prefix("env.") {
            env::var(var).unwrap_or_default()
        } else {
            profile.api_key.clone()
        };

        let extra_headers: Vec<(String, String)> = profile
            .headers
            .iter()
            .map(|(k, v)| {
                let resolved = crate::client::resolve::resolve_env_inline(v);
                (k.clone(), resolved)
            })
            .collect();

        let has_auth_header = extra_headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("authorization") || k.eq_ignore_ascii_case("x-api-key"));

        if raw_key.is_empty() && !has_auth_header {
            return Err(ProviderError::AuthError(
                "no API key configured".to_string(),
            ));
        }
        let base_url = if profile.base_url.is_empty() {
            API_URL.to_string()
        } else {
            format!("{}/v1/messages", profile.base_url.trim_end_matches('/'))
        };

        let model = if profile.model.is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            profile.model.clone()
        };

        Ok(AnthropicClient {
            base_client: BaseClient::new()?,
            api_url: base_url,
            api_key: raw_key,
            model,
            max_tokens: profile.max_tokens.unwrap_or(8192),
            reasoning_effort: profile.reasoning_effort.clone(),
            extra_headers,
        })
    }

    pub fn with_log(mut self, path: std::path::PathBuf) -> Self {
        self.base_client = self.base_client.with_log(path);
        self
    }

    pub fn api_client(&self) -> AnthropicApiClient<'_> {
        AnthropicApiClient { inner: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Message;
    use serde_json::Value;

    #[test]
    fn test_to_wire_message_plain() {
        let m = Message {
            role: "user".to_string(),
            content: "hello".to_string(),
            tool_calls: None,
            tool_result: None,
        };
        let w = to_wire_message(&m);
        assert_eq!(w.content, Value::String("hello".to_string()));
    }

    #[test]
    fn test_to_wire_message_tool_call() {
        use crate::types::ToolCall;
        let m = Message {
            role: "assistant".to_string(),
            content: String::new(),
            tool_calls: Some(vec![ToolCall {
                id: "tc1".to_string(),
                name: "bash".to_string(),
                input: serde_json::json!({"cmd": "ls"}),
            }]),
            tool_result: None,
        };
        let w = to_wire_message(&m);
        assert!(w.content.is_array());
        let arr = w.content.as_array().unwrap();
        assert_eq!(arr[0]["type"], "tool_use");
        assert_eq!(arr[0]["name"], "bash");
    }

    #[test]
    fn test_to_wire_message_tool_result() {
        use crate::types::ToolResult;
        let m = Message {
            role: "user".to_string(),
            content: String::new(),
            tool_calls: None,
            tool_result: Some(ToolResult {
                call_id: "tc1".to_string(),
                content: serde_json::Value::String("output".to_string()),
            }),
        };
        let w = to_wire_message(&m);
        assert!(w.content.is_array());
        let arr = w.content.as_array().unwrap();
        assert_eq!(arr[0]["type"], "tool_result");
        assert_eq!(arr[0]["tool_use_id"], "tc1");
        assert_eq!(arr[0]["content"], "output");
    }

    #[test]
    fn test_to_wire_message_tool_result_json_object() {
        use crate::types::ToolResult;
        let m = Message {
            role: "user".to_string(),
            content: String::new(),
            tool_calls: None,
            tool_result: Some(ToolResult {
                call_id: "tc2".to_string(),
                content: serde_json::json!({"foo.rs": "some content"}),
            }),
        };
        let w = to_wire_message(&m);
        let arr = w.content.as_array().unwrap();
        let content_str = arr[0]["content"].as_str().unwrap();
        assert!(content_str.contains("foo.rs"));
    }

    #[test]
    fn test_anthropic_response_deserialization() {
        let json = r#"{
            "content": [
                {"type": "text", "text": "Hello world"}
            ],
            "stop_reason": "end_turn"
        }"#;

        let response: AnthropicResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.content.len(), 1);
    }
}
