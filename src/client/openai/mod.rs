use crate::client::base::BaseClient;
use crate::provider::ProviderError;
use std::env;

const DEFAULT_API_URL: &str = "http://localhost:1234/v1/chat/completions";
const DEFAULT_MODEL: &str = "local-model";

mod wire;
mod messages;
mod tools;
mod sse;
mod api;

pub use wire::*;
pub use messages::to_openai_message;
pub use tools::openai_tool_definitions;
pub use sse::OpenAiStream;
pub use api::OpenAiApiClient;

pub struct OpenAiClient {
    base_client: BaseClient,
    api_url: String,
    model: String,
    max_tokens: u32,
    reasoning_effort: Option<String>,
    extra_headers: Vec<(String, String)>,
    auth_header: String,
}

impl OpenAiClient {
    pub fn new() -> Result<Self, ProviderError> {
        let _api_key = env::var("OPENAI_API_KEY").map_err(|_| {
            ProviderError::AuthError(
                "OPENAI_API_KEY environment variable not set".to_string(),
            )
        })?;

        let _base_client = BaseClient::new()?;

        Ok(OpenAiClient {
            base_client: BaseClient::new()?,
            api_url: DEFAULT_API_URL.to_string(),
            model: DEFAULT_MODEL.to_string(),
            max_tokens: 8192,
            reasoning_effort: None,
            extra_headers: vec![],
            auth_header: String::new(),
        })
    }

    pub fn from_profile(profile: &crate::config::Profile) -> Result<Self, ProviderError> {
        let raw_key = if profile.api_key.is_empty() {
            env::var("OPENAI_API_KEY").unwrap_or_default()
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
            .any(|(k, _)| {
                k.eq_ignore_ascii_case("authorization") || k.eq_ignore_ascii_case("api-key")
            });

        if raw_key.is_empty() && !has_auth_header {
            return Err(ProviderError::AuthError(
                "no API key configured".to_string(),
            ));
        }

        let base_url = if profile.base_url.is_empty() {
            DEFAULT_API_URL.to_string()
        } else {
            format!(
                "{}/v1/chat/completions",
                profile.base_url.trim_end_matches('/')
            )
        };

        let model = if profile.model.is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            profile.model.clone()
        };

        let auth_header = if raw_key.is_empty() && has_auth_header {
            String::new()
        } else {
            format!("Bearer {}", raw_key)
        };

        Ok(OpenAiClient {
            base_client: BaseClient::new()?,
            api_url: base_url,
            model,
            max_tokens: profile.max_tokens.unwrap_or(8192),
            reasoning_effort: profile.reasoning_effort.clone(),
            extra_headers,
            auth_header,
        })
    }

    pub fn with_log(mut self, path: std::path::PathBuf) -> Self {
        self.base_client = self.base_client.with_log(path);
        self
    }

    pub fn api_client(&self) -> OpenAiApiClient<'_> {
        OpenAiApiClient { inner: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Message, ToolCall, ToolResult};

    #[test]
    fn test_to_openai_message_plain() {
        let m = Message {
            role: "user".to_string(),
            content: "hello".to_string(),
            tool_calls: None,
            tool_result: None,
        };
        let w = to_openai_message(&m);
        assert_eq!(w.role, "user");
        assert_eq!(w.content, Some("hello".to_string()));
        assert!(w.tool_calls.is_none());
    }

    #[test]
    fn test_to_openai_message_tool_call() {
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
        let w = to_openai_message(&m);
        assert_eq!(w.role, "assistant");
        assert!(w.content.is_none());
        assert!(w.tool_call_id.is_none());
        let calls = w.tool_calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "tc1");
        assert_eq!(calls[0].kind, "function");
        assert_eq!(calls[0].function.name, "bash");
        assert_eq!(calls[0].function.arguments, r#"{"cmd":"ls"}"#);
    }

    #[test]
    fn test_to_openai_message_tool_result() {
        let m = Message {
            role: "user".to_string(),
            content: String::new(),
            tool_calls: None,
            tool_result: Some(ToolResult {
                call_id: "tc1".to_string(),
                content: serde_json::Value::String("output".to_string()),
            }),
        };
        let w = to_openai_message(&m);
        assert_eq!(w.role, "tool");
        assert_eq!(w.tool_call_id, Some("tc1".to_string()));
        assert_eq!(w.content, Some("output".to_string()));
    }

    #[test]
    fn test_to_openai_message_tool_result_json_object() {
        let m = Message {
            role: "user".to_string(),
            content: String::new(),
            tool_calls: None,
            tool_result: Some(ToolResult {
                call_id: "tc2".to_string(),
                content: serde_json::json!({"foo.rs": "some content"}),
            }),
        };
        let w = to_openai_message(&m);
        assert_eq!(w.role, "tool");
        assert!(w.content.as_ref().unwrap().contains("foo.rs"));
    }

    #[test]
    fn test_openai_tool_schema_mapping() {
        let defs = openai_tool_definitions();
        assert_eq!(defs.len(), 3);
        let names: Vec<&str> = defs
            .iter()
            .filter_map(|d| d.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()))
            .collect();
        assert!(names.contains(&"bash"));
        assert!(names.contains(&"fs_read"));
        assert!(names.contains(&"fs_edit"));

        for def in &defs {
            assert_eq!(def["type"].as_str().unwrap(), "function");
            assert!(def["function"]["parameters"].is_object());
        }
    }

    #[test]
    fn test_openai_response_deserialization() {
        let json = r#"{
            "choices": [{"message": {"content": "Hello world"}}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        }"#;

        let response: OpenAiListResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].message.content, Some("Hello world".to_string()));
        assert_eq!(response.usage.as_ref().unwrap().prompt_tokens, Some(10));
    }

    #[test]
    fn test_openai_response_with_tool_calls() {
        let json = r#"{
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "call-1",
                        "type": "function",
                        "function": {"name": "bash", "arguments": "{\"cmd\":\"ls\"}"}
                    }]
                }
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        }"#;

        let response: OpenAiListResponse = serde_json::from_str(json).unwrap();
        let tc = &response.choices[0].message.tool_calls.as_ref().unwrap()[0];
        assert_eq!(tc.id, "call-1");
        assert_eq!(tc.function.name, "bash");
        assert_eq!(tc.function.arguments, r#"{"cmd":"ls"}"#);
    }
}
