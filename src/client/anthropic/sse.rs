use crate::client::sse::{SseEventMapper, SseParser};
use serde_json::Value;
use crate::provider::{ProviderError, StreamEvent};
use std::io::BufRead;

/// Provider-specific mapper for Anthropic SSE events.
pub struct AnthropicEventMapper {
    model: Option<String>,
    /// Accumulate output_tokens from message_delta events.
    pending_output_tokens: Option<u32>,
}

impl AnthropicEventMapper {
    pub fn new() -> Self {
        AnthropicEventMapper {
            model: None,
            pending_output_tokens: None,
        }
    }
}

impl SseEventMapper for AnthropicEventMapper {
    fn map_event(
        &mut self,
        chunk: crate::client::sse::ParsedChunk,
        text_buf: &mut String,
        reasoning_buf: &mut String,
        tool_calls: &mut Vec<crate::client::sse::ToolCallAccumulator>,
    ) -> Option<Vec<StreamEvent>> {
        match chunk {
            crate::client::sse::ParsedChunk::Eof => None,
            crate::client::sse::ParsedChunk::Json(data) => {
                self.handle_anthropic_data(
                    &data,
                    text_buf,
                    reasoning_buf,
                    tool_calls,
                )
            }
        }
    }
}

impl AnthropicEventMapper {
    fn handle_anthropic_data(
        &mut self,
        data: &Value,
        text_buf: &mut String,
        reasoning_buf: &mut String,
        tool_calls: &mut Vec<crate::client::sse::ToolCallAccumulator>,
    ) -> Option<Vec<StreamEvent>> {
        // message_start — model name
        if data["type"].as_str() == Some("message_start") {
            if let Some(m) = data["message"]["model"].as_str() {
                self.model = Some(m.to_string());
            }
            return Some(vec![]);
        }

        // message_delta — usage info (output_tokens)
        if data["type"].as_str() == Some("message_delta") {
            if let Some(usage) = data["usage"].as_object() {
                if let Some(output_tokens) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
                    self.pending_output_tokens = Some(output_tokens as u32);
                }
            }
            return Some(vec![]);
        }

        // content_block_start
        if data["type"].as_str() == Some("content_block_start") {
            let block_type = data["content_block"]["type"].as_str().unwrap_or("");
            let index = data["index"].as_u64().unwrap_or(0) as usize;

            if block_type == "thinking" {
                *reasoning_buf = String::new();
                return Some(vec![]);
            }
            if block_type == "tool_use" {
                let id = data["content_block"]["id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let name = data["content_block"]["name"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                tool_calls.push(crate::client::sse::ToolCallAccumulator {
                    index,
                    id,
                    name: name.clone(),
                    input_json: String::new(),
                });
                return Some(vec![StreamEvent::ToolCallDelta {
                    index,
                    name,
                }]);
            }
            return Some(vec![]);
        }

        // content_block_delta
        if data["type"].as_str() == Some("content_block_delta") {
            let index = data["index"].as_u64().unwrap_or(0) as usize;
            let delta_type = data["delta"]["type"].as_str().unwrap_or("");

            match delta_type {
                "text_delta" => {
                    let text = data["delta"]["text"].as_str().unwrap_or("");
                    text_buf.push_str(text);
                    return Some(vec![StreamEvent::TextDelta(text.to_string())]);
                }
                "thinking_delta" => {
                    let text = data["delta"]["thinking"].as_str().unwrap_or("");
                    reasoning_buf.push_str(text);
                    return Some(vec![StreamEvent::ReasoningDelta(text.to_string())]);
                }
                "input_json_delta" => {
                    let partial = data["delta"]["partial_json"].as_str().unwrap_or("");
                    if let Some(tc) = tool_calls.iter_mut().find(|tc| tc.index == index) {
                        tc.input_json.push_str(partial);
                    }
                    let name = tool_calls
                        .iter()
                        .find(|tc| tc.index == index)
                        .map(|tc| tc.name.clone())
                        .unwrap_or_default();
                    return Some(vec![StreamEvent::ToolCallDelta {
                        index,
                        name,
                    }]);
                }
                "stop_reason" => {
                    return Some(vec![]);
                }
                _ => return Some(vec![]),
            }
        }

        // message_stop — finalize
        if data["type"].as_str() == Some("message_stop") {
            return Some(vec![]);
        }

        // error — signal failure
        if data["error"].is_object() {
            return None;
        }

        Some(vec![])
    }

    #[allow(dead_code)]
    fn finalize_usage(&mut self, usage: Option<crate::types::TokenUsage>) -> Option<crate::types::TokenUsage> {
        match (usage, self.pending_output_tokens) {
            (Some(mut u), Some(output_tokens)) => {
                u.output_tokens = output_tokens;
                Some(u)
            }
            (u, _) => u,
        }
    }

    #[allow(dead_code)]
    fn finalize_model(&mut self, model: Option<String>) -> Option<String> {
        self.model.take().or(model)
    }
}

/// Drives an Anthropic SSE stream, yielding `StreamEvent`s.
pub struct SseStream<R: BufRead> {
    inner: SseParser<R, AnthropicEventMapper>,
}

impl<R: BufRead> SseStream<R> {
    pub fn new(reader: R) -> Self {
        SseStream {
            inner: SseParser::new(reader, AnthropicEventMapper::new()),
        }
    }
}

impl<R: BufRead> Iterator for SseStream<R> {
    type Item = Result<StreamEvent, ProviderError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_whitespace_only_text_becomes_none() {
        let data = "event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"\\n\\n\"}}\n\n";
        let reader = Cursor::new(data.as_bytes().to_vec());
        let mut stream = SseStream::new(reader);

        // Drain all events until Complete
        while let Some(event) = stream.next() {
            match event {
                Ok(StreamEvent::Complete(resp)) => {
                    assert!(
                        resp.message.is_none(),
                        "whitespace-only text should result in None message, got: {:?}",
                        resp.message
                    );
                }
                _ => {}
            }
        }
    }
}
