use serde_json::Value;

use super::wire::OpenAiFunctionSchema;

/// Convert Anthropic tool definitions to OpenAI format.
pub fn openai_tool_definitions() -> Vec<Value> {
    crate::tools::tool_definitions()
        .into_iter()
        .map(|tool| {
            let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let desc = tool.get("description").and_then(|d| d.as_str()).unwrap_or("");
            let input_schema = tool.get("input_schema").cloned().unwrap_or_default();
            serde_json::json!({
                "type": "function",
                "function": OpenAiFunctionSchema {
                    name: name.to_string(),
                    description: if desc.is_empty() { None } else { Some(desc.to_string()) },
                    parameters: input_schema,
                }
            })
        })
        .collect()
}
