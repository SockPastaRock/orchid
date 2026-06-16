use crate::tools::scope::is_in_scope;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::process::{Command, Stdio};

#[derive(Deserialize)]
pub struct BashInput {
    pub cmd: String,
}

pub fn execute(
    input: Value,
    working_dir: &str,
    allow_scope_escape: bool,
    env_vars: &HashMap<String, String>,
) -> Result<String, String> {
    let bash_input: BashInput =
        serde_json::from_value(input).map_err(|e| format!("invalid bash input: {}", e))?;

    if !allow_scope_escape {
        validate_cmd_scope(&bash_input.cmd, working_dir)?;
    }

    let output = Command::new("bash")
        .arg("-c")
        .arg(&bash_input.cmd)
        .current_dir(working_dir)
        .envs(env_vars)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to execute command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let combined = if stderr.is_empty() {
        stdout.to_string()
    } else if stdout.is_empty() {
        stderr.to_string()
    } else {
        format!("{}{}", stdout, stderr)
    };

    Ok(combined)
}

fn validate_cmd_scope(cmd: &str, working_dir: &str) -> Result<(), String> {
    let tokens = tokenize_cmd(cmd);
    for token in tokens {
        if token.starts_with("-") {
            continue;
        }
        if token.starts_with("|")
            || token.starts_with(";")
            || token.starts_with("&&")
            || token.starts_with("||")
            || token.starts_with(">")
            || token.starts_with("<")
        {
            continue;
        }
        if !is_in_scope(&token, working_dir) && !token.contains("/") && !is_builtin(&token) {
            continue;
        }
        if token.contains("/") && !is_in_scope(&token, working_dir) {
            return Err(format!("path out of scope: {}", token));
        }
    }
    Ok(())
}

fn tokenize_cmd(cmd: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut quote_char = ' ';

    for ch in cmd.chars() {
        match ch {
            '\'' | '"' => {
                if in_quote && ch == quote_char {
                    in_quote = false;
                } else if !in_quote {
                    in_quote = true;
                    quote_char = ch;
                } else {
                    current.push(ch);
                }
            }
            ' ' | '\t' if !in_quote => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn is_builtin(cmd: &str) -> bool {
    matches!(
        cmd,
        "echo"
            | "cd"
            | "pwd"
            | "ls"
            | "cat"
            | "grep"
            | "find"
            | "wc"
            | "sort"
            | "head"
            | "tail"
            | "sed"
            | "awk"
            | "tr"
            | "cut"
            | "test"
            | "["
            | "true"
            | "false"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_simple() {
        let input = serde_json::json!({"cmd": "echo hello"});
        let result = execute(input, "/tmp", false, &std::collections::HashMap::new());
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("hello"));
    }

    #[test]
    fn test_execute_with_stderr() {
        let input = serde_json::json!({"cmd": "echo error >&2"});
        let result = execute(input, "/tmp", false, &std::collections::HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_tokenize_simple() {
        let tokens = tokenize_cmd("ls -la /tmp");
        assert_eq!(tokens, vec!["ls", "-la", "/tmp"]);
    }

    #[test]
    fn test_tokenize_quoted() {
        let tokens = tokenize_cmd("echo 'hello world'");
        assert_eq!(tokens, vec!["echo", "hello world"]);
    }

    #[test]
    fn test_is_builtin() {
        assert!(is_builtin("echo"));
        assert!(is_builtin("ls"));
        assert!(!is_builtin("nonexistent"));
    }
}
