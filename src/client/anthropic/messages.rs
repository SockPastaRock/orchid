use crate::types::Message;
use serde_json::Value;
use super::wire::AnthropicMessage;

/// Convert an orchid Message into an Anthropic wire message.
///
/// - Regular user/assistant messages: `content` is a plain string.
/// - Tool call (assistant): `content` is `[{type:"tool_use", id, name, input}]`.
/// - Tool result (user):    `content` is `[{type:"tool_result", tool_use_id, content}]`.
pub fn to_wire_message(m: &Message) -> AnthropicMessage {
    if let Some(tool_calls) = &m.tool_calls {
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
