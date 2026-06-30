use crate::types::{Message, TokenUsage, ToolCall};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Incremental event emitted during a streaming provider response.
pub enum StreamEvent {
    /// A text delta from the assistant.
    TextDelta(String),
    /// A tool call being accumulated (name known, input still arriving).
    ToolCallDelta { index: usize, name: String },
    /// Incremental reasoning/thinking content from the assistant.
    ReasoningDelta(String),
    /// Stream finished — carries the fully assembled response.
    Complete(Response),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Token usage returned by the provider for this step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    /// Model name used for this step (as reported by provider).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ProviderError {
    Network(String),
    InvalidResponse(String),
    AuthError(String),
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ProviderError::Network(msg) => write!(f, "network error: {}", msg),
            ProviderError::InvalidResponse(msg) => write!(f, "invalid response: {}", msg),
            ProviderError::AuthError(msg) => write!(f, "auth error: {}", msg),
        }
    }
}

pub trait Provider: Send + Sync {
    fn send(&self, system: String, messages: Vec<Message>) -> Result<Response, ProviderError>;

    fn send_streaming(
        &self,
        system: String,
        messages: Vec<Message>,
    ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, ProviderError>>>, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_serialize() {
        let resp = Response {
            message: Some("hello".to_string()),
            reasoning: None,
            tool_calls: None,
            usage: None,
            model: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("hello"));
        assert!(!json.contains("tool_calls"));
    }

    #[test]
    fn test_provider_error_display() {
        let err = ProviderError::Network("connection failed".to_string());
        assert_eq!(err.to_string(), "network error: connection failed");
    }
}
