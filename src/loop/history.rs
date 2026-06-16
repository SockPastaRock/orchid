use crate::get_convo_jsonl_path;
use crate::log::{DiagLogger, LogReader};
use crate::tools::fs_read::extract_paths;
use crate::types::{ConvoEvent, Message, ToolResult};
use serde_json::Value;
use std::collections::HashMap;

pub fn build_message_history(convo_id: &str, log: &DiagLogger) -> Result<Vec<Message>, String> {
    let path = get_convo_jsonl_path(convo_id)?;

    if !std::path::Path::new(&path).exists() {
        return Ok(Vec::new());
    }

    let all_events = LogReader::read_lines(&path)?;

    // Pass 1: find the last index at which each file path was read.
    let mut last_read: HashMap<String, usize> = HashMap::new();

    for (idx, event) in all_events.iter().enumerate() {
        if let ConvoEvent::ToolCall(e) = event {
            for tc in &e.tool_call.calls {
                if tc.name == "fs_read" {
                    for p in extract_paths(&tc.input) {
                        last_read.insert(p, idx);
                    }
                }
            }
        }
    }

    // Pass 2: build the message slice, substituting stale content markers in-memory.
    let mut call_map: HashMap<String, (String, serde_json::Value)> = HashMap::new();
    let mut messages = Vec::new();
    let mut raw_messages = Vec::new();
    let mut tombstone_count: u32 = 0;

    for (idx, event) in all_events.iter().enumerate() {
        match event {
            ConvoEvent::Message(e) => {
                if e.message.role != "system" {
                    let msg = Message {
                        role: e.message.role.clone(),
                        content: e.message.content.clone(),
                        tool_calls: None,
                        tool_result: None,
                    };
                    raw_messages.push(msg.clone());
                    messages.push(msg);
                }
            }
            ConvoEvent::ToolCall(e) => {
                for tc in &e.tool_call.calls {
                    call_map.insert(tc.id.clone(), (tc.name.clone(), tc.input.clone()));
                }
                let msg = Message {
                    role: "assistant".to_string(),
                    content: String::new(),
                    tool_calls: Some(e.tool_call.calls.clone()),
                    tool_result: None,
                };
                raw_messages.push(msg.clone());
                messages.push(msg);
            }
            ConvoEvent::ToolResult(e) => {
                let tr = &e.tool_result;
                let raw_msg = Message {
                    role: "user".to_string(),
                    content: String::new(),
                    tool_calls: None,
                    tool_result: Some(tr.clone()),
                };

                if let Some((name, input)) = call_map.get(&tr.call_id) {
                    if name == "fs_read" {
                        let paths = extract_paths(input);
                        let stale_paths: Vec<&String> = paths
                            .iter()
                            .filter(|p| last_read.get(*p).is_some_and(|&last| last > idx))
                            .collect();

                        if !stale_paths.is_empty() {
                            tombstone_count += 1;
                            raw_messages.push(raw_msg);
                            messages.push(Message {
                                role: "user".to_string(),
                                content: String::new(),
                                tool_calls: None,
                                tool_result: Some(ToolResult {
                                    call_id: tr.call_id.clone(),
                                    content: replace_stale_in_value(&tr.content, &stale_paths),
                                }),
                            });
                            continue;
                        }
                    }
                }
                raw_messages.push(raw_msg.clone());
                messages.push(raw_msg);
            }
        }
    }

    if tombstone_count > 0 {
        let raw_tokens = estimate_tokens_from_messages(&raw_messages);
        let effective_tokens = estimate_tokens_from_messages(&messages);
        log.debug(
            "tombstone_savings",
            &format!(
                "tombstones={} raw_tokens={} effective_tokens={} saved={}",
                tombstone_count,
                raw_tokens,
                effective_tokens,
                raw_tokens.saturating_sub(effective_tokens),
            ),
        );
    }

    Ok(messages)
}

pub fn estimate_tokens_from_messages(messages: &[Message]) -> u32 {
    let bytes = serde_json::to_string(messages)
        .map(|s| s.len())
        .unwrap_or(0);
    (bytes / 3) as u32
}

/// Replace stale path entries in a tool result `Value` with `{"stale": true}`.
/// Content is expected to be a JSON object `{"<path>": <value>, ...}`.
/// Paths not present in the object are silently ignored.
/// Falls back to the original value unchanged if it is not an object.
fn replace_stale_in_value(content: &Value, stale_paths: &[&String]) -> Value {
    let Value::Object(map) = content else {
        return content.clone();
    };

    let mut map = map.clone();
    for path in stale_paths {
        if map.contains_key(path.as_str()) {
            map.insert(path.to_string(), serde_json::json!({"stale": true}));
        }
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::{DiagLogger, LogLevel, LogReader, LogWriter};
    use crate::types::{
        ConvoEvent, MessageEvent, ToolCall, ToolCallEvent, ToolResult, ToolResultEvent,
    };
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_build_empty_history() {
        let result = build_message_history("nonexistent-id", &DiagLogger::noop());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_build_message_history() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let convo_path = temp_dir.path().join("test-id");
        fs::create_dir(&convo_path)?;
        let jsonl_path = convo_path.join("conversation.jsonl");

        LogWriter::append(
            &jsonl_path,
            &ConvoEvent::Message(MessageEvent::new("user", "hello")),
        )?;
        LogWriter::append(
            &jsonl_path,
            &ConvoEvent::Message(MessageEvent::new("assistant", "hi there")),
        )?;

        let messages = build_message_history_from_path(&jsonl_path)?;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");

        Ok(())
    }

    #[test]
    fn test_stale_read_replacement() -> Result<(), Box<dyn std::error::Error>> {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let temp_dir = TempDir::new()?;
        let convo_id = "stale-test-001";
        let convo_path = temp_dir.path().join("conversations").join(convo_id);
        fs::create_dir_all(&convo_path)?;
        let jsonl_path = convo_path.join("conversation.jsonl");

        let tc1 = ConvoEvent::ToolCall(ToolCallEvent::new(vec![ToolCall {
            id: "c1".to_string(),
            name: "fs_read".to_string(),
            input: serde_json::json!({"paths": ["foo.rs"]}),
        }]));
        let tr1 = ConvoEvent::ToolResult(ToolResultEvent::new(ToolResult {
            call_id: "c1".to_string(),
            content: serde_json::json!({"foo.rs": "original content"}),
        }));
        let tc2 = ConvoEvent::ToolCall(ToolCallEvent::new(vec![ToolCall {
            id: "c2".to_string(),
            name: "fs_read".to_string(),
            input: serde_json::json!({"paths": ["foo.rs"]}),
        }]));
        let tr2 = ConvoEvent::ToolResult(ToolResultEvent::new(ToolResult {
            call_id: "c2".to_string(),
            content: serde_json::json!({"foo.rs": "updated content"}),
        }));

        for e in [&tc1, &tr1, &tc2, &tr2] {
            LogWriter::append(&jsonl_path, e)?;
        }
        let disk_before = fs::read_to_string(&jsonl_path)?;

        std::env::set_var("ORCHID_DIR", temp_dir.path().to_string_lossy().to_string());
        let log = DiagLogger::for_convo(convo_path.clone(), LogLevel::Debug);
        let messages = build_message_history(convo_id, &log)?;

        let user_messages: Vec<&Message> = messages.iter().filter(|m| m.role == "user").collect();
        assert_eq!(user_messages.len(), 2);

        // In-memory: tr1 should have stale marker, tr2 should have updated content.
        let tr1_content = &user_messages[0].tool_result.as_ref().unwrap().content;
        assert_eq!(
            tr1_content["foo.rs"],
            serde_json::json!({"stale": true}),
            "expected stale marker in-memory, got: {}",
            tr1_content
        );

        let tr2_content = &user_messages[1].tool_result.as_ref().unwrap().content;
        assert_eq!(
            tr2_content["foo.rs"].as_str().unwrap(),
            "updated content",
            "expected updated content in-memory, got: {}",
            tr2_content
        );

        // On-disk: JSONL must be unchanged — no rewrite.
        let disk_after = fs::read_to_string(&jsonl_path)?;
        assert_eq!(
            disk_before, disk_after,
            "build_message_history must not rewrite the JSONL"
        );

        // Debug log must contain tombstone_savings event.
        let log_path = convo_path.join("orchid.log");
        let log_contents = fs::read_to_string(&log_path)?;
        assert!(
            log_contents.contains("tombstone_savings"),
            "expected tombstone_savings in orchid.log"
        );
        assert!(
            log_contents.contains("tombstones=1"),
            "expected tombstones=1 in log"
        );

        Ok(())
    }

    // Helper for tests that have a path directly (bypassing ORCHID_DIR resolution).
    fn build_message_history_from_path(path: &std::path::Path) -> Result<Vec<Message>, String> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let all_events = LogReader::read_lines(path)?;
        let mut messages = Vec::new();
        for event in all_events {
            match event {
                ConvoEvent::Message(e) => {
                    if e.message.role != "system" {
                        messages.push(Message {
                            role: e.message.role,
                            content: e.message.content,
                            tool_calls: None,
                            tool_result: None,
                        });
                    }
                }
                ConvoEvent::ToolCall(e) => {
                    messages.push(Message {
                        role: "assistant".to_string(),
                        content: String::new(),
                        tool_calls: Some(e.tool_call.calls),
                        tool_result: None,
                    });
                }
                ConvoEvent::ToolResult(e) => {
                    messages.push(Message {
                        role: "user".to_string(),
                        content: String::new(),
                        tool_calls: None,
                        tool_result: Some(e.tool_result),
                    });
                }
            }
        }
        Ok(messages)
    }
}
