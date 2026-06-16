use crate::get_convo_jsonl_path;
use crate::log::LogWriter;
use crate::types::{
    ConvoEvent, MessageEvent, ToolCall, ToolCallEvent, ToolResult, ToolResultEvent,
};

pub fn append_message(convo_id: &str, content: &str) -> Result<String, String> {
    let path = get_convo_jsonl_path(convo_id)?;
    let event = ConvoEvent::Message(MessageEvent::new("assistant", content));
    LogWriter::append(&path, &event)
}

pub fn append_system(convo_id: &str, content: &str) -> Result<String, String> {
    let path = get_convo_jsonl_path(convo_id)?;
    let event = ConvoEvent::Message(MessageEvent::new("system", content));
    LogWriter::append(&path, &event)
}

pub fn append_tool_call(convo_id: &str, calls: &[ToolCall]) -> Result<String, String> {
    let path = get_convo_jsonl_path(convo_id)?;
    let event = ConvoEvent::ToolCall(ToolCallEvent::new(calls.to_vec()));
    LogWriter::append(&path, &event)
}

pub fn append_tool_result(convo_id: &str, tool_result: &ToolResult) -> Result<String, String> {
    let path = get_convo_jsonl_path(convo_id)?;
    let event = ConvoEvent::ToolResult(ToolResultEvent::new(tool_result.clone()));
    LogWriter::append(&path, &event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_convo_jsonl_path() {
        let path = get_convo_jsonl_path("test-id");
        assert!(path.is_ok());
        let p = path.unwrap();
        assert!(p.to_string_lossy().contains("test-id"));
        assert!(p.to_string_lossy().contains("conversation.jsonl"));
    }
}
