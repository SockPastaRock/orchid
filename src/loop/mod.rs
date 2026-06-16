use crate::convo::Store;
use crate::get_orchid_dir;
use crate::log::{DiagLogger, LogLevel};
use crate::provider::{Provider, StreamEvent};
use crate::tools;
use crate::types::{TokenBudget, ToolResult};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub mod budget;
pub mod events;
pub mod history;
pub mod lifecycle;

struct RunGuard<'a> {
    convo_id: &'a str,
    disarmed: bool,
}

impl<'a> RunGuard<'a> {
    fn new(convo_id: &'a str) -> Self {
        Self {
            convo_id,
            disarmed: false,
        }
    }

    fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        if !self.disarmed {
            let _ = lifecycle::on_run_end(self.convo_id);
        }
    }
}

// ── Stream liveness file ────────────────────────────────────────────────────────────────

/// Manages `stream.state` inside a conversation directory.
///
/// Format: `<unix_timestamp_secs> <chunk_count>\n`
/// - Created when streaming begins, deleted on completion or drop.
/// - External tools can poll mtime or the counter for liveness.
struct StreamState {
    path: PathBuf,
    chunk_count: u64,
}

impl StreamState {
    fn create(convo_dir: &PathBuf) -> Self {
        // Read existing chunk count so the counter is monotonic across turns.
        let prior = Self::read_chunk_count(convo_dir);
        let path = convo_dir.join("stream.state");
        let mut state = StreamState { path, chunk_count: prior };
        state.tick();
        state
    }

    fn read_chunk_count(convo_dir: &PathBuf) -> u64 {
        let path = convo_dir.join("stream.state");
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.split_whitespace().nth(1).and_then(|n| n.parse().ok()))
            .unwrap_or(0)
    }

    fn tick(&mut self) {
        self.chunk_count += 1;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Ok(mut f) = fs::File::create(&self.path) {
            let _ = write!(f, "{} {}\n", ts, self.chunk_count);
        }
    }
}



// ── Run loop ─────────────────────────────────────────────────────────────────────

pub fn run(convo_id: &str, provider: &dyn Provider, budget: &TokenBudget) -> Result<(), String> {
    let store = Store::new()?;
    let meta = store.get(convo_id)?;
    let config = crate::load_config()?;

    let convo_dir = get_orchid_dir()?.join("conversations").join(convo_id);
    let log_level = LogLevel::from_config_str(config.log_level.as_deref());
    let log = DiagLogger::for_convo(convo_dir.clone(), log_level);

    if lifecycle::detect_crashed(convo_id)? {
        let stale_pid = meta
            .pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        log.info(
            "run_crashed",
            &format!("pid={} stale — reconciling", stale_pid),
        );
        lifecycle::reconcile_crashed(convo_id)?;
    }

    log.info("run_start", convo_id);

    lifecycle::on_run_start(convo_id)?;
    let mut guard = RunGuard::new(convo_id);

    let working_dir = meta.working_dir.unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "/tmp".to_string())
    });
    let allow_scope_escape = meta.allow_scope_escape.unwrap_or(false);
    let env_vars = config
        .env_file
        .as_deref()
        .map(crate::config::load_env_file)
        .unwrap_or_default();
    let persona_name = meta.persona.as_deref().unwrap_or("default");
    let system_prompt = resolve_system_prompt(persona_name, &config)?;
    let effective_budget = resolve_persona_budget(persona_name, budget, &config);

    let warn_interval = (effective_budget
        .hard_limit
        .saturating_sub(effective_budget.warn_threshold))
        / 10;
    let mut last_warn_tokens: Option<u32> = None;

    loop {
        let messages = history::build_message_history(convo_id, &log)?;

        let estimated_tokens = history::estimate_tokens_from_messages(&messages);
        if estimated_tokens >= effective_budget.hard_limit {
            log.warn(
                "pre_send_budget_exceeded",
                &format!(
                    "estimated={} hard_limit={}",
                    estimated_tokens, effective_budget.hard_limit
                ),
            );
            let termination_msg = format!(
                "[SESSION TERMINATED] Estimated token count ({}) would exceed hard limit ({}) before sending. \
                Start a new conversation to continue.",
                estimated_tokens, effective_budget.hard_limit
            );
            events::append_system(convo_id, &termination_msg)?;
            let updates = crate::MetadataUpdate {
                last_message: Some(termination_msg.clone()),
                token_estimate: Some(estimated_tokens),
                ..Default::default()
            };
            store.update(convo_id, updates)?;
            guard.disarm();
            lifecycle::on_run_end(convo_id)?;
            log.info("run_end", "pre_send_budget_exceeded");
            return Err(format!(
                "token hard limit would be exceeded before sending: {} estimated tokens",
                estimated_tokens
            ));
        }

        log.info("provider_send", &format!("messages={}", messages.len()));

        let mut stream_state = StreamState::create(&convo_dir);
        let response = {
            let event_iter = provider
                .send_streaming(system_prompt.clone(), messages)
                .map_err(|e| {
                    log.error("provider_error", &e.to_string());
                    format!("provider error: {}", e)
                })?;

            let mut result = None;
            for event in event_iter {
                match event {
                    Err(e) => {
                        log.error("stream_error", &e.to_string());
                        return Err(format!("provider error: {}", e));
                    }
                    Ok(StreamEvent::TextDelta(_)) | Ok(StreamEvent::ToolCallDelta { .. }) => {
                        stream_state.tick();
                    }
                    Ok(StreamEvent::Complete(resp)) => {
                        result = Some(resp);
                        break;
                    }
                }
            }
            result.ok_or_else(|| "stream ended without a Complete event".to_string())?
        };

        if let Some(ref u) = response.usage {
            log.info(
                "usage",
                &format!("in={} out={}", u.input_tokens, u.output_tokens),
            );
        }

        {
            let updates = crate::MetadataUpdate {
                token_estimate: Some(estimated_tokens),
                ..Default::default()
            };
            store.update(convo_id, updates)?;
        }

        if estimated_tokens >= effective_budget.hard_limit {
            log.warn(
                "token_budget_exceeded",
                &format!(
                    "total={} hard_limit={}",
                    estimated_tokens, effective_budget.hard_limit
                ),
            );
            let termination_msg = format!(
                "[SESSION TERMINATED] Token hard limit reached ({} / {} tokens). \
                The run has been stopped. Start a new conversation to continue.",
                estimated_tokens, effective_budget.hard_limit
            );
            events::append_system(convo_id, &termination_msg)?;
            let updates = crate::MetadataUpdate {
                last_message: Some(termination_msg),
                ..Default::default()
            };
            store.update(convo_id, updates)?;
            guard.disarm();
            lifecycle::on_run_end(convo_id)?;
            log.info("run_end", "budget_exceeded");
            return Err(format!(
                "token hard limit exceeded: {} tokens",
                estimated_tokens
            ));
        } else if estimated_tokens >= effective_budget.warn_threshold {
            let should_warn = match last_warn_tokens {
                None => true,
                Some(last) => estimated_tokens >= last.saturating_add(warn_interval),
            };
            if should_warn {
                last_warn_tokens = Some(estimated_tokens);
                log.warn(
                    "token_budget_warning",
                    &format!(
                        "total={} warn_threshold={}",
                        estimated_tokens, effective_budget.warn_threshold
                    ),
                );
                events::append_system(
                    convo_id,
                    &format!(
                        "[WARNING] This session has consumed {} tokens (warn threshold: {}). \
                        Consider wrapping up or the session will be terminated at {} tokens.",
                        estimated_tokens,
                        effective_budget.warn_threshold,
                        effective_budget.hard_limit
                    ),
                )?;
            }
        }

        if let Some(tool_calls) = response.tool_calls {
            if let Some(ref msg) = response.message {
                if !msg.is_empty() {
                    events::append_message(convo_id, msg)?;
                }
            }

            for tool_call in tool_calls {
                log.info(
                    "tool_call",
                    &format!("tool={} id={}", tool_call.name, tool_call.id),
                );
                events::append_tool_call(convo_id, std::slice::from_ref(&tool_call))?;

                let content = match tools::execute_tool(
                    &tool_call.name,
                    tool_call.input.clone(),
                    &working_dir,
                    allow_scope_escape,
                    &env_vars,
                ) {
                    Ok(raw) => {
                        log.info("tool_result", &format!("tool={}", tool_call.name));
                        raw
                    }
                    Err(e) => {
                        log.error("tool_error", &format!("tool={} err={}", tool_call.name, e));
                        serde_json::Value::String(format!("Error: {}", e))
                    }
                };

                let tool_result = ToolResult {
                    call_id: tool_call.id,
                    content,
                };

                events::append_tool_result(convo_id, &tool_result)?;
            }
        } else if let Some(message) = response.message {
            log.info("run_complete", "");
            events::append_message(convo_id, &message)?;

            let updates = crate::MetadataUpdate {
                last_message: Some(message),
                ..Default::default()
            };
            store.update(convo_id, updates)?;

            break;
        } else {
            log.warn("empty_response", "");
            break;
        }
    }

    lifecycle::on_run_end(convo_id)?;
    guard.disarm();
    log.info("run_end", convo_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convo::Store;
    use crate::log::LogWriter;
    use crate::provider::{Provider, ProviderError, Response, StreamEvent};
    use crate::types::{ConvoEvent, Message, MessageEvent, ToolCall};
    use std::fs;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    struct MockProvider {
        responses: Arc<Mutex<Vec<Response>>>,
    }

    impl Provider for MockProvider {
        fn send(
            &self,
            _system: String,
            _messages: Vec<Message>,
        ) -> Result<Response, ProviderError> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                Err(ProviderError::Network("no more responses".to_string()))
            } else {
                Ok(responses.remove(0))
            }
        }

        fn send_streaming(
            &self,
            system: String,
            messages: Vec<Message>,
        ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, ProviderError>>>, ProviderError> {
            let response = self.send(system, messages)?;
            Ok(Box::new(std::iter::once(Ok(StreamEvent::Complete(response)))))
        }
    }

    fn setup_orchid_dir(temp: &TempDir) -> std::path::PathBuf {
        let dir = temp.path().to_path_buf();
        let prompts_dir = dir.join("system-prompts");
        fs::create_dir_all(&prompts_dir).unwrap();
        fs::write(prompts_dir.join("base.md"), "You are a helpful assistant.").unwrap();
        let config = serde_json::json!({
            "active_profile": "test",
            "profiles": {
                "test": {"provider": "anthropic", "api_key": "x", "model": "m"}
            },
            "personas": {
                "default": {"prompts": ["base"]}
            }
        });
        fs::write(dir.join("config.json"), config.to_string()).unwrap();
        dir
    }

    fn create_seeded_convo(orchid_dir: &std::path::Path) -> String {
        let convos_dir = orchid_dir.join("conversations");
        fs::create_dir_all(&convos_dir).unwrap();
        let store = Store::with_base(convos_dir);
        let meta = store
            .create(None, Some("/tmp".to_string()), None, None)
            .unwrap();

        let jsonl = orchid_dir
            .join("conversations")
            .join(&meta.id)
            .join("conversation.jsonl");
        let event = ConvoEvent::Message(MessageEvent::new("user", "do something"));
        LogWriter::append(&jsonl, &event).unwrap();
        meta.id
    }

    #[test]
    fn test_tool_error_returned_to_model_not_propagated() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = TempDir::new().unwrap();
        let orchid_dir = setup_orchid_dir(&temp);
        std::env::set_var("ORCHID_DIR", orchid_dir.to_string_lossy().to_string());

        let convo_id = create_seeded_convo(&orchid_dir);

        // Step 1: model requests fs_read on /etc/passwd — out of scope for working_dir=/tmp.
        let step1 = Response {
            message: None,
            tool_calls: Some(vec![ToolCall {
                id: "call-1".to_string(),
                name: "fs_read".to_string(),
                input: serde_json::json!({"paths": ["/etc/passwd"]}),
            }]),
            usage: None,
            model: None,
        };
        // Step 2: model receives the error and replies with a final message.
        let step2 = Response {
            message: Some("I cannot access that file.".to_string()),
            tool_calls: None,
            usage: None,
            model: None,
        };

        let provider = MockProvider {
            responses: Arc::new(Mutex::new(vec![step1, step2])),
        };

        let budget = TokenBudget::default();
        let result = run(&convo_id, &provider, &budget);

        assert!(
            result.is_ok(),
            "run should complete despite tool error, got: {:?}",
            result
        );

        let jsonl = orchid_dir
            .join("conversations")
            .join(&convo_id)
            .join("conversation.jsonl");
        let contents = fs::read_to_string(&jsonl).unwrap();

        assert!(
            contents.contains("out of scope"),
            "tool error should appear in conversation log, got:\n{}",
            contents
        );
        assert!(
            contents.contains("I cannot access that file."),
            "model recovery message should appear in conversation log, got:\n{}",
            contents
        );
    }

    #[test]
    fn test_provider_error_leaves_convo_idle() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = TempDir::new().unwrap();
        let orchid_dir = setup_orchid_dir(&temp);
        std::env::set_var("ORCHID_DIR", orchid_dir.to_string_lossy().to_string());

        let convo_id = create_seeded_convo(&orchid_dir);

        // Provider returns no responses — first send fails with a network error.
        let provider = MockProvider {
            responses: Arc::new(Mutex::new(vec![])),
        };

        let budget = TokenBudget::default();
        let result = run(&convo_id, &provider, &budget);

        assert!(result.is_err(), "run should fail when provider errors");

        let store = Store::with_base(orchid_dir.join("conversations"));
        let meta = store.get(&convo_id).unwrap();
        assert_eq!(
            meta.status,
            crate::types::Status::Idle,
            "conversation must be Idle after provider error, was {:?}",
            meta.status
        );
    }

    #[test]
    fn test_persona_budget_override() {
        let config_json = serde_json::json!({
            "active_profile": "test",
            "profiles": {
                "test": {"provider": "anthropic", "api_key": "x", "model": "m"}
            },
            "personas": {
                "default": {"prompts": ["base"]},
                "high-limit": {
                    "prompts": ["base"],
                    "limits": {
                        "token_warn_threshold": 100000,
                        "token_hard_limit": 160000
                    }
                }
            }
        });
        let config: crate::Config = serde_json::from_value(config_json).unwrap();
        let global = crate::types::TokenBudget::default();

        let default_budget = resolve_persona_budget("default", &global, &config);
        assert_eq!(
            default_budget.warn_threshold, global.warn_threshold,
            "default persona should use global limits"
        );

        let high_budget = resolve_persona_budget("high-limit", &global, &config);
        assert_eq!(high_budget.warn_threshold, 100_000);
        assert_eq!(high_budget.hard_limit, 160_000);
    }

    #[test]
    fn test_pre_send_budget_exceeded_does_not_call_provider() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = TempDir::new().unwrap();
        let orchid_dir = setup_orchid_dir(&temp);
        std::env::set_var("ORCHID_DIR", orchid_dir.to_string_lossy().to_string());

        let convo_id = create_seeded_convo(&orchid_dir);

        // Provider should never be called — if it is, the test fails with "no more responses".
        let provider = MockProvider {
            responses: Arc::new(Mutex::new(vec![])),
        };

        // Set hard_limit=1 so any non-empty history exceeds it before sending.
        let budget = TokenBudget {
            hard_limit: 1,
            warn_threshold: 0,
        };
        let result = run(&convo_id, &provider, &budget);

        assert!(
            result.is_err(),
            "run should fail when pre-send budget is exceeded"
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("token hard limit would be exceeded before sending"),
            "error message should indicate pre-send guard, got: {}",
            err
        );

        let store = Store::with_base(orchid_dir.join("conversations"));
        let meta = store.get(&convo_id).unwrap();
        assert_eq!(
            meta.status,
            crate::types::Status::Idle,
            "conversation must be Idle after pre-send termination, was {:?}",
            meta.status
        );
    }
}

fn resolve_system_prompt(persona_name: &str, config: &crate::Config) -> Result<String, String> {
    let personas = match config.extra.get("personas") {
        Some(v) => v,
        None => return Err("no personas defined in config".to_string()),
    };

    let persona = match personas.get(persona_name) {
        Some(v) => v,
        None => return Err(format!("persona '{}' not found in config", persona_name)),
    };

    let prompts = persona
        .get("prompts")
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("persona '{}' has no prompts array", persona_name))?;

    let prompts_dir = crate::get_orchid_dir()?.join("system-prompts");

    let mut parts = Vec::new();
    for p in prompts {
        let name = p
            .as_str()
            .ok_or_else(|| "prompt name must be a string".to_string())?;
        let path = prompts_dir.join(format!("{}.md", name));
        let content = std::fs::read_to_string(&path)
            .map_err(|_| format!("system prompt file not found: {}", path.display()))?;
        parts.push(content.trim().to_string());
    }

    Ok(parts.join("\n\n"))
}

/// Read persona-level limits from config and merge over the global limits.
/// Persona limits shadow global ones only for fields that are explicitly set.
fn resolve_persona_budget(
    persona_name: &str,
    global: &crate::types::TokenBudget,
    config: &crate::Config,
) -> crate::types::TokenBudget {
    let persona_limits = config
        .extra
        .get("personas")
        .and_then(|p| p.get(persona_name))
        .and_then(|p| p.get("limits"));

    let Some(limits) = persona_limits else {
        return global.clone();
    };

    let warn_threshold = limits
        .get("token_warn_threshold")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(global.warn_threshold);

    let hard_limit = limits
        .get("token_hard_limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(global.hard_limit);

    crate::types::TokenBudget {
        warn_threshold,
        hard_limit,
    }
}
