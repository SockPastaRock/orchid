use crate::types::Message;
use super::wire::{OpenAiFunction, OpenAiMessage, OpenAiToolCall};

/// Convert an orchid Message into an OpenAI wire message.
///
/// - Plain user/assistant: `{role, content}`
/// - Tool call (assistant): `{role, tool_calls: [{id, type: "function", function: {name, arguments}}]}`
/// - Tool result (user): `{role: "tool", tool_call_id, content}`
pub fn to_openai_message(m: &Message) -> OpenAiMessage {
    if let Some(tool_calls) = &m.tool_calls {
        let calls: Vec<OpenAiToolCall> = tool_calls
            .iter()
            .map(|tc| {
                OpenAiToolCall {
                    id: tc.id.clone(),
                    kind: "function".to_string(),
                    function: OpenAiFunction {
                        name: tc.name.clone(),
                        arguments: tc.input.to_string(),
                    },
                }
            })
            .collect();
        return OpenAiMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(calls),
            tool_call_id: None,
        };
    }

    if let Some(tr) = &m.tool_result {
        let content_str = match &tr.content {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        return OpenAiMessage {
            role: "tool".to_string(),
            content: Some(content_str),
            tool_calls: None,
            tool_call_id: Some(tr.call_id.clone()),
        };
    }

    OpenAiMessage {
        role: m.role.clone(),
        content: Some(m.content.clone()),
        tool_calls: None,
        tool_call_id: None,
    }
}
