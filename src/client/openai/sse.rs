use crate::provider::{ProviderError, Response, StreamEvent};
use crate::types::{TokenUsage, ToolCall};
use std::io::BufRead;

/// Accumulator for a single tool call being built across streaming deltas.
struct StreamToolCallAccumulator {
    id: Option<String>,
    name: String,
    arguments_json: String,
}

/// Drives an OpenAI SSE stream, yielding `StreamEvent`s.
pub struct OpenAiStream<R: BufRead> {
    reader: R,
    done: bool,
    text_buf: String,
    reasoning_buf: String,
    tool_calls: Vec<StreamToolCallAccumulator>,
    usage: Option<TokenUsage>,
}

impl<R: BufRead> OpenAiStream<R> {
    pub fn new(reader: R) -> Self {
        OpenAiStream {
            reader,
            done: false,
            text_buf: String::new(),
            reasoning_buf: String::new(),
            tool_calls: Vec::new(),
            usage: None,
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
                        id: tc.id.clone().unwrap_or_default(),
                        name: tc.name.clone(),
                        input: serde_json::from_str(&tc.arguments_json)
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
            model: None,
        }
    }
}

impl<R: BufRead> Iterator for OpenAiStream<R> {
    type Item = Result<StreamEvent, ProviderError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

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
                continue;
            }

            if let Some(rest) = line.strip_prefix("data: ") {
                data_line = rest.to_string();
            }
        }

        if data_line.trim() == "[DONE]" {
            self.done = true;
            return Some(Ok(StreamEvent::Complete(self.build_complete_response())));
        }

        let chunk: super::wire::OpenAiStreamChunk = match serde_json::from_str(&data_line) {
            Ok(v) => v,
            Err(e) => {
                return Some(Err(ProviderError::InvalidResponse(format!(
                    "SSE JSON parse error: {} — data: {}",
                    e, &data_line[..data_line.len().min(200)]
                ))));
            }
        };

        if let Some(ref usage) = chunk.usage {
            self.usage = Some(TokenUsage {
                input_tokens: usage.prompt_tokens.unwrap_or(0),
                output_tokens: usage.completion_tokens.unwrap_or(0),
            });
            return self.next();
        }

        let choices = match chunk.choices {
            Some(c) => c,
            None => return self.next(),
        };

        if choices.is_empty() {
            return self.next();
        }

        let delta = match &choices[0].delta {
            Some(d) => d,
            None => return self.next(),
        };

        if let Some(ref text) = delta.content {
            if !text.is_empty() {
                self.text_buf.push_str(text);
                return Some(Ok(StreamEvent::TextDelta(text.clone())));
            }
        }

        if let Some(ref reasoning) = delta.reasoning_content {
            if !reasoning.is_empty() {
                self.reasoning_buf.push_str(reasoning);
            }
        }

        if let Some(ref calls) = delta.tool_calls {
            if calls.is_empty() {
                return self.next();
            }

            for call in calls {
                let idx = call.index.unwrap_or(0) as usize;

                if let Some(ref id) = call.id {
                    if idx >= self.tool_calls.len() {
                        self.tool_calls.push(StreamToolCallAccumulator {
                            id: Some(id.clone()),
                            name: call.function.as_ref().and_then(|f| f.name.clone()).unwrap_or_default(),
                            arguments_json: String::new(),
                        });
                    } else if self.tool_calls[idx].id.is_none() {
                        self.tool_calls[idx].id = Some(id.clone());
                    }
                }

                if let Some(ref func) = call.function {
                    if let Some(ref args) = func.arguments {
                        if idx < self.tool_calls.len() {
                            self.tool_calls[idx].arguments_json.push_str(args);
                        }
                    }
                    if let Some(ref name) = func.name {
                        if idx < self.tool_calls.len() {
                            return Some(Ok(StreamEvent::ToolCallDelta {
                                index: idx,
                                name: name.clone(),
                            }));
                        }
                    }
                }
            }
        }

        if let Some(ref finish_reason) = choices[0].finish_reason {
            if finish_reason == "stop" || finish_reason == "tool_calls" {
                self.done = true;
                return Some(Ok(StreamEvent::Complete(self.build_complete_response())));
            }
        }

        self.next()
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
