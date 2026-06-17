use crate::provider::{ProviderError, Response, StreamEvent};
use crate::types::{TokenUsage, ToolCall};
use serde_json::Value;
use std::io::BufRead;

/// Accumulator for a single tool_use block being built across deltas.
struct ToolCallAccumulator {
    index: usize,
    id: String,
    name: String,
    input_json: String,
}

/// Drives an Anthropic SSE stream, yielding `StreamEvent`s.
pub struct SseStream<R: BufRead> {
    reader: R,
    done: bool,
    text_buf: String,
    reasoning_buf: String,
    tool_calls: Vec<ToolCallAccumulator>,
    usage: Option<TokenUsage>,
    model: Option<String>,
}

impl<R: BufRead> SseStream<R> {
    pub fn new(reader: R) -> Self {
        SseStream {
            reader,
            done: false,
            text_buf: String::new(),
            reasoning_buf: String::new(),
            tool_calls: Vec::new(),
            usage: None,
            model: None,
        }
    }

    fn build_complete_response(&self) -> Response {
        let message = if self.text_buf.trim().is_empty() {
            None
        } else {
            Some(self.text_buf.clone())
        };

        let reasoning = if self.reasoning_buf.trim().is_empty() {
            None
        } else {
            Some(self.reasoning_buf.clone())
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
            reasoning,
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

        let mut event_type = String::new();
        let mut data_line = String::new();

        loop {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => {
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
                if !data_line.is_empty() {
                    break;
                }
                event_type.clear();
                continue;
            }

            if let Some(rest) = line.strip_prefix("event: ") {
                event_type = rest.to_string();
            } else if let Some(rest) = line.strip_prefix("data: ") {
                data_line = rest.to_string();
            }
        }

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
                self.next()
            }

            "content_block_start" => {
                let index = data["index"].as_u64().unwrap_or(0) as usize;
                let block_type = data["content_block"]["type"].as_str().unwrap_or("");

                if block_type == "thinking" {
                    // Start accumulating reasoning/thinking content
                    self.reasoning_buf = String::new();
                    self.next()
                } else if block_type == "tool_use" {
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
                    "thinking_delta" => {
                        let text = data["delta"]["thinking"].as_str().unwrap_or("");
                        self.reasoning_buf.push_str(text);
                        self.next()
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
                    "stop_reason" => {
                        self.next()
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

            _ => self.next(),
        }
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
