use crate::client::anthropic::{AnthropicClient, AnthropicMessage, AnthropicResponse, ContentBlock};
use crate::client::anthropic::{SseStream, to_wire_message};
use crate::provider::{Provider, ProviderError, Response, StreamEvent};
use crate::types::{TokenUsage, ToolCall};

use std::io::BufReader;

pub struct AnthropicApiClient<'a> {
    pub inner: &'a AnthropicClient,
}

impl Provider for AnthropicClient {
    fn send(&self, system: String, messages: Vec<crate::types::Message>) -> Result<Response, ProviderError> {
        let api = self.api_client();
        api.send(system, messages)
    }

    fn send_streaming(
        &self,
        system: String,
        messages: Vec<crate::types::Message>,
    ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, ProviderError>>>, ProviderError> {
        let api = self.api_client();
        api.send_streaming(system, messages)
    }
}

impl<'a> AnthropicApiClient<'a> {
    pub fn send(
        &self,
        system: String,
        messages: Vec<crate::types::Message>,
    ) -> Result<Response, ProviderError> {
        let body = self.build_request_body(system, messages, false)?;

        let mut headers: Vec<(&str, &str)> = vec![("anthropic-version", "2023-06-01")];
        if !self.inner.api_key.is_empty() {
            headers.push(("x-api-key", &self.inner.api_key));
        }
        let extra: Vec<(&str, &str)> = self
            .inner
            .extra_headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        headers.extend_from_slice(&extra);

        let response_text = self
            .inner
            .base_client
            .post_with_retry(&self.inner.api_url, body, &headers)?;

        self.parse_response(&response_text)
    }

    pub fn send_streaming(
        &self,
        system: String,
        messages: Vec<crate::types::Message>,
    ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, ProviderError>>>, ProviderError> {
        let body = self.build_request_body(system, messages, true)?;

        let mut headers: Vec<(&str, &str)> = vec![("anthropic-version", "2023-06-01")];
        if !self.inner.api_key.is_empty() {
            headers.push(("x-api-key", &self.inner.api_key));
        }
        let extra: Vec<(&str, &str)> = self
            .inner
            .extra_headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        headers.extend_from_slice(&extra);

        let masked: Vec<String> = headers
            .iter()
            .map(|(k, v)| {
                let tail = if v.len() > 4 { &v[v.len() - 4..] } else { v };
                format!("{}=...{}", k, tail)
            })
            .collect();
        self.inner.base_client.log_debug("resolved_headers", &masked.join(", "));

        let response = self
            .inner
            .base_client
            .post_streaming(&self.inner.api_url, body, &headers)?;

        Ok(Box::new(SseStream::new(BufReader::new(response))))
    }

    fn build_request_body(
        &self,
        system: String,
        messages: Vec<crate::types::Message>,
        stream: bool,
    ) -> Result<String, ProviderError> {
        let wire_messages: Vec<AnthropicMessage> = messages.iter().map(to_wire_message).collect();
        let mut body = serde_json::json!({
            "model": self.inner.model,
            "max_tokens": self.inner.max_tokens,
            "system": system,
            "messages": wire_messages,
            "tools": crate::tools::tool_definitions(),
        });
        if let Some(ref effort) = self.inner.reasoning_effort {
            if effort != "none" {
                // Map reasoning_effort to Anthropic's thinking budget.
                // The higher the effort, the more budget tokens we allocate.
                let budget: u32 = match effort.as_str() {
                    "minimal" | "low" => 1024,
                    "medium" => 2048,
                    "high" | "xhigh" => 4096,
                    _ => 1024,
                };
                body["thinking"] = serde_json::json!({
                    "type": "enabled",
                    "budget_tokens": budget,
                });
            }
        }
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
        let mut reasoning_text: Option<String> = None;
        let mut tool_calls_vec: Vec<ToolCall> = Vec::new();

        for block in anthropic_response.content {
            match block {
                ContentBlock::Text { text } => {
                    if !text.is_empty() {
                        message_text = Some(text);
                    }
                }
                ContentBlock::Thinking { text } => {
                    if !text.is_empty() {
                        reasoning_text = Some(text);
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
            reasoning: reasoning_text,
            tool_calls,
            usage,
            model: anthropic_response.model,
        })
    }
}
