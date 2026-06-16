use crate::client::base::BaseClient;
use crate::provider::{Provider, ProviderError, Response, StreamEvent};
use crate::tools::tool_definitions;
use crate::types::{Message, TokenUsage, ToolCall};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::io::{BufRead, BufReader};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MODEL: &str = "claude-3-5-sonnet-20241022";

/// Anthropic wire message — content can be a plain string or an array of content blocks.
#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: Value, // String or Array
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    model: Option<String>,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

pub struct AnthropicClient {
    base_client: BaseClient,
    api_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
    /// Extra headers resolved from the profile (e.g. portkey gateway headers).
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

        // Resolve env.* indirection in profile headers.
        // Supports both whole-value (`env.VAR`) and inline (`Bearer env.VAR`) forms.
        let extra_headers: Vec<(String, String)> = profile
            .headers
            .iter()
            .map(|(k, v)| {
                let resolved = resolve_env_inline(v);
                (k.clone(), resolved)
            })
            .collect();

        // Require an api_key unless the profile supplies its own auth via headers.
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
            extra_headers,
        })
    }

    /// Attach a log file path so `BaseClient` can write debug entries.
    pub fn with_log(mut self, path: std::path::PathBuf) -> Self {
        self.base_client = self.base_client.with_log(path);
        self
    }
}

/// Convert an orchid Message into an Anthropic wire message.
///
/// - Regular user/assistant messages: `content` is a plain string.
/// - Tool call (assistant): `content` is `[{type:"tool_use", id, name, input}]`.
/// - Tool result (user):    `content` is `[{type:"tool_result", tool_use_id, content}]`.
/// Replace all `env.<VAR>` tokens in a string with the corresponding env var value.
/// Handles both whole-value (`env.FOO`) and inline (`Bearer env.FOO`) forms.
fn resolve_env_inline(s: &str) -> String {
    let mut result = s.to_string();
    // Find each `env.` occurrence and replace the token (word chars after the dot).
    while let Some(start) = result.find("env.") {
        let after = &result[start + 4..];
        let end = after
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after.len());
        let var_name = &after[..end];
        let value = env::var(var_name).unwrap_or_default();
        result = format!("{}{}{}", &result[..start], value, &after[end..]);
    }
    result
}

fn to_wire_message(m: &Message) -> AnthropicMessage {
    if let Some(tool_calls) = &m.tool_calls {
        // One tool call per message in our model (loop appends them individually).
        let blocks: Vec<Value> = tool_calls
            .iter()
            .map(|tc| {
                serde_json::json!({
                    "type": "tool_use",
                    "id": tc.id,
                    "name": tc.name,
                    "input": tc.input
                })
            })
            .collect();
        return AnthropicMessage {
            role: "assistant".to_string(),
            content: Value::Array(blocks),
        };
    }

    if let Some(tr) = &m.tool_result {
        let content_str = match &tr.content {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        return AnthropicMessage {
            role: "user".to_string(),
            content: Value::Array(vec![serde_json::json!({
                "type": "tool_result",
                "tool_use_id": tr.call_id,
                "content": content_str
            })]),
        };
    }

    AnthropicMessage {
        role: m.role.clone(),
        content: Value::String(m.content.clone()),
    }
}

impl Provider for AnthropicClient {
    fn send(&self, system: String, messages: Vec<Message>) -> Result<Response, ProviderError> {
        let body = self.build_request_body(system, messages, false)?;

        let mut headers: Vec<(&str, &str)> = vec![("anthropic-version", "2023-06-01")];
        if !self.api_key.is_empty() {
            headers.push(("x-api-key", &self.api_key));
        }
        let extra: Vec<(&str, &str)> = self
            .extra_headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        headers.extend_from_slice(&extra);

        let response_text = self
            .base_client
            .post_with_retry(&self.api_url, body, &headers)?;

        self.parse_response(&response_text)
    }

    fn send_streaming(
        &self,
        system: String,
        messages: Vec<Message>,
    ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, ProviderError>>>, ProviderError> {
        let body = self.build_request_body(system, messages, true)?;

        let mut headers: Vec<(&str, &str)> = vec![("anthropic-version", "2023-06-01")];
        if !self.api_key.is_empty() {
            headers.push(("x-api-key", &self.api_key));
        }
        let extra: Vec<(&str, &str)> = self
            .extra_headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        headers.extend_from_slice(&extra);

        // Log resolved header values (masked) to confirm env resolution.
        let masked: Vec<String> = headers
            .iter()
            .map(|(k, v)| {
                let tail = if v.len() > 4 { &v[v.len() - 4..] } else { v };
                format!("{}=...{}", k, tail)
            })
            .collect();
        self.base_client.log_debug("resolved_headers", &masked.join(", "));

        let response = self
            .base_client
            .post_streaming(&self.api_url, body, &headers)?;

        Ok(Box::new(SseStream::new(BufReader::new(response))))
    }
}

impl AnthropicClient {
    fn build_request_body(
        &self,
        system: String,
        messages: Vec<Message>,
        stream: bool,
    ) -> Result<String, ProviderError> {
        let wire_messages: Vec<AnthropicMessage> = messages.iter().map(to_wire_message).collect();
        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "system": system,
            "messages": wire_messages,
            "tools": tool_definitions(),
        });
        if stream {
            body["stream"] = serde_json::json!(true);
        }
        serde_json::to_string(&body)
            .map_err(|e| ProviderError::InvalidResponse(format!("failed to serialize request: {}", e)))
    }

    fn parse_response(&self, response_text: &str) -> Result<Response, ProviderError> {
        let anthropic_response: AnthropicResponse =
            serde_json::from_str(response_text).map_err(|e| {
                ProviderError::InvalidResponse(format!(
                    "failed to parse response: {} — raw: {}",
                    e,
                    &response_text[..response_text.len().min(500)]
                ))
            })?;

        let mut message_text: Option<String> = None;
        let mut tool_calls_vec: Vec<ToolCall> = Vec::new();

        for block in anthropic_response.content {
            match block {
                ContentBlock::Text { text } => {
                    if !text.is_empty() {
                        message_text = Some(text);
                    }
                }
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls_vec.push(ToolCall { id, name, input });
                }
            }
        }

        let tool_calls = if tool_calls_vec.is_empty() {
            None
        } else {
            Some(tool_calls_vec)
        };

        let usage = anthropic_response.usage.map(|u| TokenUsage {
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
        });

        Ok(Response {
            message: message_text,
            tool_calls,
            usage,
            model: anthropic_response.model,
        })
    }
}

// ── SSE streaming iterator ────────────────────────────────────────────────────

/// Accumulator for a single tool_use block being built across deltas.
struct ToolCallAccumulator {
    index: usize,
    id: String,
    name: String,
    input_json: String,
}

/// Drives an Anthropic SSE stream, yielding `StreamEvent`s.
struct SseStream<R: BufRead> {
    reader: R,
    done: bool,
    // Accumulator state
    text_buf: String,
    tool_calls: Vec<ToolCallAccumulator>,
    usage: Option<TokenUsage>,
    model: Option<String>,
}

impl<R: BufRead> SseStream<R> {
    fn new(reader: R) -> Self {
        SseStream {
            reader,
            done: false,
            text_buf: String::new(),
            tool_calls: Vec::new(),
            usage: None,
            model: None,
        }
    }

    fn build_complete_response(&self) -> Response {
        let message = if self.text_buf.is_empty() {
            None
        } else {
            Some(self.text_buf.clone())
        };

        let tool_calls = if self.tool_calls.is_empty() {
            None
        } else {
            Some(
                self.tool_calls
                    .iter()
                    .map(|tc| ToolCall {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        input: serde_json::from_str(&tc.input_json)
                            .unwrap_or(serde_json::Value::Null),
                    })
                    .collect(),
            )
        };

        Response {
            message,
            tool_calls,
            usage: self.usage.clone(),
            model: self.model.clone(),
        }
    }
}

impl<R: BufRead> Iterator for SseStream<R> {
    type Item = Result<StreamEvent, ProviderError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        // Read SSE lines until we have a complete event (blank-line terminated).
        // We only care about `data:` lines; `event:` lines are used for routing.
        let mut event_type = String::new();
        let mut data_line = String::new();

        loop {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => {
                    // EOF without message_stop — treat as completion.
                    self.done = true;
                    return Some(Ok(StreamEvent::Complete(self.build_complete_response())));
                }
                Err(e) => {
                    self.done = true;
                    return Some(Err(ProviderError::Network(e.to_string())));
                }
                Ok(_) => {}
            }

            let line = line.trim_end_matches(|c: char| c == '\n' || c == '\r');

            if line.is_empty() {
                // Blank line = end of SSE event block.
                if !data_line.is_empty() {
                    break;
                }
                // Empty event (heartbeat/comment only) — keep reading.
                event_type.clear();
                continue;
            }

            if let Some(rest) = line.strip_prefix("event: ") {
                event_type = rest.to_string();
            } else if let Some(rest) = line.strip_prefix("data: ") {
                data_line = rest.to_string();
            }
            // Ignore `id:` and `:` (SSE comments / keepalives).
        }

        // Parse the JSON data payload.
        let data: Value = match serde_json::from_str(&data_line) {
            Ok(v) => v,
            Err(e) => {
                return Some(Err(ProviderError::InvalidResponse(format!(
                    "SSE JSON parse error: {} — data: {}",
                    e, &data_line[..data_line.len().min(200)]
                ))));
            }
        };

        match event_type.as_str() {
            "message_start" => {
                if let Some(model) = data["message"]["model"].as_str() {
                    self.model = Some(model.to_string());
                }
                // Recurse to next event.
                self.next()
            }

            "content_block_start" => {
                let index = data["index"].as_u64().unwrap_or(0) as usize;
                let block_type = data["content_block"]["type"].as_str().unwrap_or("");

                if block_type == "tool_use" {
                    let id = data["content_block"]["id"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    let name = data["content_block"]["name"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    self.tool_calls.push(ToolCallAccumulator {
                        index,
                        id,
                        name: name.clone(),
                        input_json: String::new(),
                    });

                    Some(Ok(StreamEvent::ToolCallDelta { index, name }))
                } else {
                    self.next()
                }
            }

            "content_block_delta" => {
                let index = data["index"].as_u64().unwrap_or(0) as usize;
                let delta_type = data["delta"]["type"].as_str().unwrap_or("");

                match delta_type {
                    "text_delta" => {
                        let text = data["delta"]["text"].as_str().unwrap_or("");
                        self.text_buf.push_str(text);
                        Some(Ok(StreamEvent::TextDelta(text.to_string())))
                    }
                    "input_json_delta" => {
                        let partial = data["delta"]["partial_json"].as_str().unwrap_or("");
                        if let Some(tc) = self.tool_calls.iter_mut().find(|tc| tc.index == index) {
                            tc.input_json.push_str(partial);
                        }
                        let name = self
                            .tool_calls
                            .iter()
                            .find(|tc| tc.index == index)
                            .map(|tc| tc.name.clone())
                            .unwrap_or_default();
                        Some(Ok(StreamEvent::ToolCallDelta { index, name }))
                    }
                    _ => self.next(),
                }
            }

            "message_delta" => {
                if let Some(usage) = data["usage"].as_object() {
                    let output_tokens =
                        usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    if let Some(ref mut u) = self.usage {
                        u.output_tokens = output_tokens;
                    } else {
                        self.usage = Some(TokenUsage {
                            input_tokens: 0,
                            output_tokens,
                        });
                    }
                }
                self.next()
            }

            "message_stop" => {
                self.done = true;
                Some(Ok(StreamEvent::Complete(self.build_complete_response())))
            }

            "error" => {
                self.done = true;
                let msg = data["error"]["message"]
                    .as_str()
                    .unwrap_or("unknown SSE error")
                    .to_string();
                Some(Err(ProviderError::InvalidResponse(msg)))
            }

            // ping / content_block_stop / unknown — skip.
            _ => self.next(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_env_inline_whole_value() {
        std::env::set_var("TEST_INLINE_VAR", "mytoken");
        assert_eq!(resolve_env_inline("env.TEST_INLINE_VAR"), "mytoken");
    }

    #[test]
    fn test_resolve_env_inline_with_prefix() {
        std::env::set_var("TEST_INLINE_VAR", "mytoken");
        assert_eq!(resolve_env_inline("Bearer env.TEST_INLINE_VAR"), "Bearer mytoken");
    }

    #[test]
    fn test_resolve_env_inline_unset_var() {
        std::env::remove_var("TEST_INLINE_MISSING");
        assert_eq!(resolve_env_inline("Bearer env.TEST_INLINE_MISSING"), "Bearer ");
    }

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
        // JSON object content is serialised to a compact string for the provider
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
