use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Anthropic wire message — content can be a plain string or an array of content blocks.
#[derive(Debug, Serialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: Value, // String or Array
}

#[derive(Debug, Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicResponse {
    pub content: Vec<ContentBlock>,
    pub model: Option<String>,
    pub usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}
