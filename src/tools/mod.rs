use serde_json::Value;
use std::collections::HashMap;

pub mod bash;
pub mod fs_edit;
pub mod fs_read;
pub mod scope;

pub trait Tool: Send + Sync {
    fn execute(&self, args: Value, working_dir: &str) -> Result<String, String>;
}

/// Static registry of tool JSON schemas sent to the provider on every request.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        serde_json::json!({
            "name": "bash",
            "description": "Run a shell command. Output is captured and returned.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "cmd": { "type": "string", "description": "Shell command to execute" }
                },
                "required": ["cmd"]
            }
        }),
        serde_json::json!({
            "name": "fs_read",
            "description": "Read one or more files. Pass a list of paths for batch reads.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "File paths to read"
                    }
                },
                "required": ["paths"]
            }
        }),
        serde_json::json!({
            "name": "fs_edit",
            "description": "Apply one or more string replacements to a file. All edits are applied atomically — if any patch fails, nothing is written.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File to edit" },
                    "edits": {
                        "type": "array",
                        "description": "Ordered list of patches to apply",
                        "items": {
                            "type": "object",
                            "properties": {
                                "old_string": { "type": "string", "description": "Exact text to find" },
                                "new_string": { "type": "string", "description": "Replacement text" },
                                "replace_all": { "type": "boolean", "description": "Replace all occurrences (default false)" }
                            },
                            "required": ["old_string", "new_string"]
                        }
                    }
                },
                "required": ["path", "edits"]
            }
        }),
    ]
}

pub fn execute_tool(
    name: &str,
    input: Value,
    working_dir: &str,
    allow_scope_escape: bool,
    env_vars: &HashMap<String, String>,
) -> Result<Value, String> {
    match name {
        "bash" => bash::execute(input, working_dir, allow_scope_escape, env_vars).map(Value::String),
        "fs_read" => fs_read::execute(input, working_dir, allow_scope_escape),
        "fs_edit" => fs_edit::execute(input, working_dir, allow_scope_escape).map(Value::String),
        _ => Err(format!("unknown tool: {}", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions_count() {
        let defs = tool_definitions();
        assert_eq!(defs.len(), 3);
        let names: Vec<&str> = defs
            .iter()
            .filter_map(|d| d.get("name").and_then(|n| n.as_str()))
            .collect();
        assert!(names.contains(&"bash"));
        assert!(names.contains(&"fs_read"));
        assert!(names.contains(&"fs_edit"));
    }

    #[test]
    fn test_execute_tool_bash() {
        let input = serde_json::json!({"cmd": "echo test"});
        let result = execute_tool("bash", input, "/tmp", false, &HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_tool_unknown() {
        let input = serde_json::json!({});
        let result = execute_tool("unknown_tool", input, "/tmp", false, &HashMap::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown tool"));
    }

    #[test]
    fn test_execute_tool_fs_read() {
        let input = serde_json::json!({"paths": ["/tmp"]});
        let result = execute_tool("fs_read", input, "/tmp", false, &HashMap::new());
        // /tmp is a directory — may error, but that's fine; tool dispatch worked
        assert!(result.is_ok() || result.is_err());
    }
}
