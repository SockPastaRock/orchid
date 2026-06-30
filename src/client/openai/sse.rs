use crate::client::sse::{SseEventMapper, SseParser};
use crate::provider::{ProviderError, StreamEvent};
use std::io::BufRead;

/// Provider-specific mapper for OpenAI SSE events.
pub struct OpenAiEventMapper {
    pub finish_reason: Option<String>,
}

impl OpenAiEventMapper {
    pub fn new() -> Self {
        OpenAiEventMapper { finish_reason: None }
    }
}

impl SseEventMapper for OpenAiEventMapper {
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
                self.handle_openai_data(&data, text_buf, reasoning_buf, tool_calls)
            }
        }
    }
}

impl OpenAiEventMapper {
    fn handle_openai_data(
        &mut self,
        data: &serde_json::Value,
        text_buf: &mut String,
        reasoning_buf: &mut String,
        tool_calls: &mut Vec<crate::client::sse::ToolCallAccumulator>,
    ) -> Option<Vec<StreamEvent>> {
        // OpenAI wire format: OpenAiStreamChunk
        let choices = match data["choices"].as_array() {
            Some(c) if !c.is_empty() => c,
            _ => return Some(vec![]),
        };

        let choice = &choices[0];

        // Check finish_reason for stream termination
        if let Some(ref finish_reason) = choice["finish_reason"].as_str() {
            if *finish_reason == "stop" || *finish_reason == "tool_calls" {
                self.finish_reason = Some(finish_reason.to_string());
            }
        }

        let delta = match choice["delta"].as_object() {
            Some(d) => d,
            None => return Some(vec![]),
        };

        // content
        if let Some(text) = delta["content"].as_str() {
            if !text.is_empty() {
                text_buf.push_str(text);
                return Some(vec![StreamEvent::TextDelta(text.to_string())]);
            }
        }

        // reasoning_content (o1/o3 style)
        if let Some(reasoning) = delta["reasoning_content"].as_str() {
            if !reasoning.is_empty() {
                reasoning_buf.push_str(reasoning);
                return Some(vec![StreamEvent::ReasoningDelta(reasoning.to_string())]);
            }
        }

        // tool_calls
        if let Some(calls) = data["choices"][0]["delta"]["tool_calls"].as_array() {
            if calls.is_empty() {
                return Some(vec![]);
            }

            let mut events = Vec::new();
            for call in calls {
                let idx = call["index"].as_u64().unwrap_or(0) as usize;

                if let Some(ref id) = call["id"].as_str() {
                    if idx >= tool_calls.len() {
                        let name = call["function"]["name"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        tool_calls.push(crate::client::sse::ToolCallAccumulator {
                            index: idx,
                            id: id.to_string(),
                            name: name.to_string(),
                            input_json: String::new(),
                        });
                        events.push(StreamEvent::ToolCallDelta {
                            index: idx,
                            name,
                        });
                    } else if tool_calls[idx].id.is_empty() {
                        tool_calls[idx].id = id.to_string();
                    }
                }

                if let Some(ref func) = call["function"].as_object() {
                    if let Some(ref args) = func["arguments"].as_str() {
                        if idx < tool_calls.len() {
                            tool_calls[idx].input_json.push_str(args);
                        }
                    }
                    if let Some(ref name) = func["name"].as_str() {
                        if idx < tool_calls.len() && !name.is_empty() {
                            events.push(StreamEvent::ToolCallDelta {
                                index: idx,
                                name: name.to_string(),
                            });
                        }
                    }
                }
            }
            return Some(events);
        }

        Some(vec![])
    }
}

/// Drives an OpenAI SSE stream, yielding `StreamEvent`s.
pub struct OpenAiStream<R: BufRead> {
    inner: SseParser<R, OpenAiEventMapper>,
}

impl<R: BufRead> OpenAiStream<R> {
    pub fn new(reader: R) -> Self {
        OpenAiStream {
            inner: SseParser::new(reader, OpenAiEventMapper::new()),
        }
    }
}

impl<R: BufRead> Iterator for OpenAiStream<R> {
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
        let data = b"event: message_delta\ndata: {\"delta\":{\"content\":\"\\n\\n\"}}\n\ndata: [DONE]\n";
        let reader = Cursor::new(data.to_vec());
        let mut stream = OpenAiStream::new(reader);

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
