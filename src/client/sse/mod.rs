use crate::provider::{ProviderError, Response, StreamEvent};
use std::io::BufRead;

pub struct ToolCallAccumulator {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub input_json: String,
}

pub enum ParsedChunk {
    Eof,
    Json(serde_json::Value),
}

/// Trait for provider-specific SSE event mapping.
/// The mapper handles provider-specific JSON parsing and event emission.
/// The parser handles the shared SSE wire protocol (line reading, event/data pairing,
/// JSON parsing, EOF detection).
pub trait SseEventMapper {
    /// Map a parsed JSON chunk into zero or more `StreamEvent`s.
    /// Returns `None` to signal a fatal error (e.g. provider error JSON).
    fn map_event(
        &mut self,
        chunk: ParsedChunk,
        text_buf: &mut String,
        reasoning_buf: &mut String,
        tool_calls: &mut Vec<ToolCallAccumulator>,
    ) -> Option<Vec<StreamEvent>>;

    /// Called by the parser during `build_complete_response` to inject
    /// provider-specific usage data (e.g. Anthropic's `message_delta.usage`).
    fn finalize_usage(&mut self, usage: Option<crate::types::TokenUsage>) -> Option<crate::types::TokenUsage> {
        usage
    }

    /// Called by the parser during `build_complete_response` to inject
    /// provider-specific model data (e.g. Anthropic's `message_start.model`).
    fn finalize_model(&mut self, model: Option<String>) -> Option<String> {
        model
    }
}

pub struct SseParser<R: BufRead, M: SseEventMapper> {
    reader: R,
    done: bool,
    mapper: M,
    text_buf: String,
    reasoning_buf: String,
    tool_calls: Vec<ToolCallAccumulator>,
    model: Option<String>,
    usage: Option<crate::types::TokenUsage>,
}

impl<R: BufRead, M: SseEventMapper> SseParser<R, M> {
    pub fn new(reader: R, mapper: M) -> Self {
        SseParser {
            reader,
            done: false,
            mapper,
            text_buf: String::new(),
            reasoning_buf: String::new(),
            tool_calls: Vec::new(),
            model: None,
            usage: None,
        }
    }

    fn build_complete_response(&mut self) -> Result<Response, String> {
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
            let parsed: Result<Vec<crate::types::ToolCall>, String> = self
                .tool_calls
                .iter()
                .map(|tc| {
                    serde_json::from_str(&tc.input_json).map_err(|e| {
                        format!(
                            "tool_call '{}' (index {}) malformed JSON: {} — raw: '{}'",
                            tc.name,
                            tc.index,
                            e,
                            &tc.input_json[..tc.input_json.len().min(200)]
                        )
                    })
                    .map(|val| crate::types::ToolCall {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        input: val,
                    })
                })
                .collect();
            Some(parsed?)
        };
        let usage = self.mapper.finalize_usage(self.usage.clone());
        let model = self.mapper.finalize_model(self.model.clone());
        Ok(Response {
            message,
            reasoning,
            tool_calls,
            usage,
            model,
        })
    }

    fn read_event_pair(&mut self) -> Result<Option<(String, String)>, ProviderError> {
        let mut event_type = String::new();
        let mut data_line = String::new();
        loop {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => return Ok(None),
                Err(e) => return Err(ProviderError::Network(e.to_string())),
                Ok(_) => {}
            }
            let line = line.trim_end_matches(['\n', '\r']);
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
        Ok(Some((event_type, data_line)))
    }

    fn parse_json(&self, data_line: &str) -> Result<serde_json::Value, ProviderError> {
        serde_json::from_str(data_line).map_err(|e| {
            ProviderError::InvalidResponse(format!(
                "SSE JSON parse error: {} — data: {}",
                e,
                &data_line[..data_line.len().min(200)]
            ))
        })
    }

    fn check_eof_marker(&self, data_line: &str) -> bool {
        data_line.trim() == "[DONE]"
    }

    pub fn with_usage(mut self, usage: crate::types::TokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }
}

impl<R: BufRead, M: SseEventMapper> Iterator for SseParser<R, M> {
    type Item = Result<StreamEvent, ProviderError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let event_pair = match self.read_event_pair() {
            Ok(pair) => pair,
            Err(e) => {
                self.done = true;
                return Some(Err(e));
            }
        };
        match event_pair {
            None => {
                self.done = true;
                match self.build_complete_response() {
                    Ok(resp) => Some(Ok(StreamEvent::Complete(resp))),
                    Err(e) => Some(Err(ProviderError::InvalidResponse(e))),
                }
            }
            Some((_event_type, data_line)) => {
                if self.check_eof_marker(&data_line) {
                    self.done = true;
                    match self.build_complete_response() {
                        Ok(resp) => Some(Ok(StreamEvent::Complete(resp))),
                        Err(e) => Some(Err(ProviderError::InvalidResponse(e))),
                    }
                } else {
                    let chunk = match self.parse_json(&data_line) {
                        Ok(v) => ParsedChunk::Json(v),
                        Err(e) => {
                            self.done = true;
                            return Some(Err(e));
                        }
                    };
                    match self.mapper.map_event(
                        chunk,
                        &mut self.text_buf,
                        &mut self.reasoning_buf,
                        &mut self.tool_calls,
                    ) {
                        Some(events) => {
                            let mut iter = events.into_iter();
                            iter.next().map(Ok).or_else(|| self.next())
                        }
                        None => self.next(),
                    }
                }
            }
        }
    }
}
