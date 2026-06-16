use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Idle,
    Running,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub content: serde_json::Value,
}

// ── Conversation event types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePayload {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallPayload {
    pub calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub message: MessagePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub tool_call: ToolCallPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub tool_result: ToolResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConvoEvent {
    Message(MessageEvent),
    ToolCall(ToolCallEvent),
    ToolResult(ToolResultEvent),
}

impl MessageEvent {
    pub fn new(role: &str, content: &str) -> Self {
        Self {
            event_type: "message".to_string(),
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            message: MessagePayload {
                role: role.to_string(),
                content: content.to_string(),
            },
        }
    }
}

impl ToolCallEvent {
    pub fn new(calls: Vec<ToolCall>) -> Self {
        Self {
            event_type: "tool_call".to_string(),
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            tool_call: ToolCallPayload { calls },
        }
    }
}

impl ToolResultEvent {
    pub fn new(tool_result: ToolResult) -> Self {
        Self {
            event_type: "tool_result".to_string(),
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            tool_result,
        }
    }
}

// ── Legacy message type — used only by history→provider reconstruction ────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<ToolResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    /// Emit a warning in the log when cumulative tokens exceed this.
    pub warn_threshold: u32,
    /// Kill the run and insert a system message when cumulative tokens exceed this.
    pub hard_limit: u32,
}

impl TokenBudget {
    pub fn from_limits(limits: &crate::config::Limits) -> Self {
        TokenBudget {
            warn_threshold: limits.token_warn_threshold.unwrap_or(80_000),
            hard_limit: limits.token_hard_limit.unwrap_or(120_000),
        }
    }
}

impl Default for TokenBudget {
    fn default() -> Self {
        TokenBudget::from_limits(&crate::config::Limits::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persona: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: Status,
    pub pid: Option<u32>,
    pub run_started_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_estimate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_scope_escape: Option<bool>,
}
