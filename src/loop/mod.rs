pub mod budget;
pub mod events;
pub mod guard;
pub mod history;
pub mod lifecycle;
pub mod resolve;
pub mod run;
pub mod stream;

pub use run::run;
pub use run::run_loop;
pub use run::build_context;
pub use run::build_context_with_budget;
pub use budget::TokenBudget;
pub use resolve::resolve_persona_budget;

#[cfg(test)]
mod tests {
    use super::run::run;
    use super::run::run_loop;
    use super::run::build_context_with_budget;
    use super::resolve::resolve_persona_budget;
    use super::budget::TokenBudget;
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
            .expect("TEST_ENV_LOCK poisoned - a prior test panicked. Check test order.");
        let temp = TempDir::new().unwrap();
        let orchid_dir = setup_orchid_dir(&temp);
        std::env::set_var("ORCHID_DIR", orchid_dir.to_string_lossy().to_string());

        let convo_id = create_seeded_convo(&orchid_dir);

        // Step 1: model requests fs_read on /etc/passwd — out of scope for working_dir=/tmp.
        let step1 = Response {
            message: None,
            reasoning: None,
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
            reasoning: None,
            tool_calls: None,
            usage: None,
            model: None,
        };

        let provider = MockProvider {
            responses: Arc::new(Mutex::new(vec![step1, step2])),
        };

        let budget = TokenBudget::default();
        let mut ctx = build_context_with_budget(&convo_id, &budget).unwrap();
        let result = run_loop(&mut ctx, &provider);

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
            .expect("TEST_ENV_LOCK poisoned - a prior test panicked. Check test order.");
        let temp = TempDir::new().unwrap();
        let orchid_dir = setup_orchid_dir(&temp);
        std::env::set_var("ORCHID_DIR", orchid_dir.to_string_lossy().to_string());

        let convo_id = create_seeded_convo(&orchid_dir);

        // Provider returns no responses — first send fails with a network error.
        let provider = MockProvider {
            responses: Arc::new(Mutex::new(vec![])),
        };

        let budget = TokenBudget::default();
        let mut ctx = build_context_with_budget(&convo_id, &budget).unwrap();
        let result = run_loop(&mut ctx, &provider);

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
        let global = TokenBudget::default();

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
    fn test_empty_response_continues_loop_instead_of_breaking() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .expect("TEST_ENV_LOCK poisoned - a prior test panicked. Check test order.");
        let temp = TempDir::new().unwrap();
        let orchid_dir = setup_orchid_dir(&temp);
        std::env::set_var("ORCHID_DIR", orchid_dir.to_string_lossy().to_string());

        let convo_id = create_seeded_convo(&orchid_dir);

        // Step 1: model returns empty response (no message, no tool_calls).
        let step1 = Response {
            message: None,
            reasoning: None,
            tool_calls: None,
            usage: None,
            model: None,
        };
        // Step 2: model recovers with a final message.
        let step2 = Response {
            message: Some("I apologize for the empty response. How can I help?".to_string()),
            reasoning: None,
            tool_calls: None,
            usage: None,
            model: None,
        };

        let provider = MockProvider {
            responses: Arc::new(Mutex::new(vec![step1, step2])),
        };

        let result = run(&convo_id, &provider);

        assert!(
            result.is_ok(),
            "run should complete despite empty response, got: {:?}",
            result
        );

        let jsonl = orchid_dir
            .join("conversations")
            .join(&convo_id)
            .join("conversation.jsonl");
        let contents = fs::read_to_string(&jsonl).unwrap();

        assert!(
            contents.contains("empty response") || contents.contains("no text and no tool calls"),
            "empty response system message should appear in conversation log, got:\n{}",
            contents
        );
        assert!(
            contents.contains("I apologize for the empty response"),
            "model recovery message should appear in conversation log, got:\n{}",
            contents
        );
    }

    #[test]
    fn test_whitespace_only_message_triggers_retry() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .expect("TEST_ENV_LOCK poisoned - a prior test panicked. Check test order.");
        let temp = TempDir::new().unwrap();
        let orchid_dir = setup_orchid_dir(&temp);
        std::env::set_var("ORCHID_DIR", orchid_dir.to_string_lossy().to_string());

        let convo_id = create_seeded_convo(&orchid_dir);

        // Step 1: model returns a whitespace-only message (e.g., "\n\n").
        let step1 = Response {
            message: Some("\n\n".to_string()),
            reasoning: None,
            tool_calls: None,
            usage: None,
            model: None,
        };
        // Step 2: model recovers with an actual message.
        let step2 = Response {
            message: Some("Sorry about that. How can I help?".to_string()),
            reasoning: None,
            tool_calls: None,
            usage: None,
            model: None,
        };

        let provider = MockProvider {
            responses: Arc::new(Mutex::new(vec![step1, step2])),
        };

        let result = run(&convo_id, &provider);

        assert!(
            result.is_ok(),
            "run should complete despite whitespace-only response, got: {:?}",
            result
        );

        let jsonl = orchid_dir
            .join("conversations")
            .join(&convo_id)
            .join("conversation.jsonl");
        let contents = fs::read_to_string(&jsonl).unwrap();

        assert!(
            contents.contains("no text and no tool calls"),
            "whitespace-only response should trigger retry, got:\n{}",
            contents
        );
        assert!(
            contents.contains("Sorry about that. How can I help?"),
            "model recovery message should appear, got:\n{}",
            contents
        );
    }

    #[test]
    fn test_pre_send_budget_exceeded_does_not_call_provider() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .expect("TEST_ENV_LOCK poisoned - a prior test panicked. Check test order.");
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
        let mut ctx = build_context_with_budget(&convo_id, &budget).unwrap();
        let result = run_loop(&mut ctx, &provider);

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
