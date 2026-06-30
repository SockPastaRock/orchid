use crate::get_orchid_dir;
use crate::log::{DiagLogger, LogLevel};
use crate::r#loop::guard::RunGuard;
use crate::r#loop::lifecycle;
use crate::r#loop::resolve::{resolve_persona_budget, resolve_system_prompt};
use crate::r#loop::stream::StreamState;
use crate::r#loop::{events, history};
use crate::provider::{Provider, StreamEvent};
use crate::tools;
use crate::types::{TokenBudget, ToolResult};
use std::collections::HashMap;
use std::path::PathBuf;

/// Context gathered during the setup phase, passed into the main loop.
pub struct LoopContext {
    pub store: crate::convo::Store,
    pub meta: crate::types::Metadata,
    pub config: crate::config::Config,
    pub convo_dir: PathBuf,
    pub log: DiagLogger,
    pub working_dir: String,
    pub allow_scope_escape: bool,
    pub env_vars: HashMap<String, String>,
    pub persona_name: String,
    pub system_prompt: String,
    pub effective_budget: TokenBudget,
    pub warn_interval: u32,
}

/// Build the loop context from a conversation ID.
pub fn build_context(convo_id: &str) -> Result<LoopContext, String> {
    let store = crate::convo::Store::new()?;
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

    let working_dir = meta
        .working_dir
        .clone()
        .or_else(|| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .ok()
        })
        .ok_or_else(|| {
            "no working directory configured and current directory unavailable".to_string()
        })?;
    let allow_scope_escape = meta.allow_scope_escape.unwrap_or(false);
    let env_vars = config
        .env_file
        .as_deref()
        .map(crate::config::load_env_file)
        .unwrap_or_default();
    let persona_name = meta.persona.as_deref().unwrap_or("default").to_string();
    let system_prompt = resolve_system_prompt(&persona_name, &config)?;
    let effective_budget = resolve_persona_budget(&persona_name, &TokenBudget::default(), &config);

    let warn_interval = (effective_budget
        .hard_limit
        .saturating_sub(effective_budget.warn_threshold))
        / 10;

    Ok(LoopContext {
        store,
        meta,
        config,
        convo_dir,
        log,
        working_dir,
        allow_scope_escape,
        env_vars,
        persona_name,
        system_prompt,
        effective_budget,
        warn_interval,
    })
}

/// Execute the main conversation loop.
pub fn run_loop(ctx: &mut LoopContext, provider: &dyn Provider) -> Result<(), String> {
    let mut guard = RunGuard::new(&ctx.meta.id);
    let mut last_warn_tokens: Option<u32> = None;

    loop {
        let messages = history::build_message_history(&ctx.meta.id, &ctx.log)?;

        let estimated_tokens = history::estimate_tokens_from_messages(&messages);
        if estimated_tokens >= ctx.effective_budget.hard_limit {
            ctx.log.warn(
                "pre_send_budget_exceeded",
                &format!(
                    "estimated={} hard_limit={}",
                    estimated_tokens, ctx.effective_budget.hard_limit
                ),
            );
            let termination_msg = format!(
                "[SESSION TERMINATED] Estimated token count ({}) would exceed hard limit ({}) before sending. \
                Start a new conversation to continue.",
                estimated_tokens, ctx.effective_budget.hard_limit
            );
            events::append_system(&ctx.meta.id, &termination_msg)?;
            let updates = crate::convo::MetadataUpdate {
                last_message: Some(termination_msg.clone()),
                token_estimate: Some(estimated_tokens),
                ..Default::default()
            };
            ctx.store.update(&ctx.meta.id, updates)?;
            guard.disarm();
            lifecycle::on_run_end(&ctx.meta.id)?;
            ctx.log.info("run_end", "pre_send_budget_exceeded");
            return Err(format!(
                "token hard limit would be exceeded before sending: {} estimated tokens",
                estimated_tokens
            ));
        }

        ctx.log.info("provider_send", &format!("messages={}", messages.len()));

        let mut stream_state = StreamState::create(&ctx.convo_dir);
        let response = {
            let event_iter = provider
                .send_streaming(ctx.system_prompt.clone(), messages)
                .map_err(|e| {
                    ctx.log.error("provider_error", &e.to_string());
                    format!("provider error: {}", e)
                })?;

            let mut result = None;
            for event in event_iter {
                match event {
                    Err(e) => {
                        ctx.log.error("stream_error", &e.to_string());
                        return Err(format!("provider error: {}", e));
                    }
                    Ok(StreamEvent::TextDelta(_)) | Ok(StreamEvent::ToolCallDelta { .. }) | Ok(StreamEvent::ReasoningDelta(_)) => {
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
            ctx.log.info(
                "usage",
                &format!("in={} out={}", u.input_tokens, u.output_tokens),
            );
        }

        {
            let updates = crate::convo::MetadataUpdate {
                token_estimate: Some(estimated_tokens),
                ..Default::default()
            };
            ctx.store.update(&ctx.meta.id, updates)?;
        }

        if estimated_tokens >= ctx.effective_budget.hard_limit {
            ctx.log.warn(
                "token_budget_exceeded",
                &format!(
                    "total={} hard_limit={}",
                    estimated_tokens, ctx.effective_budget.hard_limit
                ),
            );
            let termination_msg = format!(
                "[SESSION TERMINATED] Token hard limit reached ({} / {} tokens). \
                The run has been stopped. Start a new conversation to continue.",
                estimated_tokens, ctx.effective_budget.hard_limit
            );
            events::append_system(&ctx.meta.id, &termination_msg)?;
            let updates = crate::convo::MetadataUpdate {
                last_message: Some(termination_msg),
                ..Default::default()
            };
            ctx.store.update(&ctx.meta.id, updates)?;
            guard.disarm();
            lifecycle::on_run_end(&ctx.meta.id)?;
            ctx.log.info("run_end", "budget_exceeded");
            return Err(format!(
                "token hard limit exceeded: {} tokens",
                estimated_tokens
            ));
        } else if estimated_tokens >= ctx.effective_budget.warn_threshold {
            let should_warn = match last_warn_tokens {
                None => true,
                Some(last) => estimated_tokens >= last.saturating_add(ctx.warn_interval),
            };
            if should_warn {
                last_warn_tokens = Some(estimated_tokens);
                ctx.log.warn(
                    "token_budget_warning",
                    &format!(
                        "total={} warn_threshold={}",
                        estimated_tokens, ctx.effective_budget.warn_threshold
                    ),
                );
                events::append_system(
                    &ctx.meta.id,
                    &format!(
                        "[WARNING] This session has consumed {} tokens (warn threshold: {}). \
                        Consider wrapping up or the session will be terminated at {} tokens.",
                        estimated_tokens,
                        ctx.effective_budget.warn_threshold,
                        ctx.effective_budget.hard_limit
                    ),
                )?;
            }
        }

        if let Some(tool_calls) = response.tool_calls {
            if let Some(ref msg) = response.message {
                if !msg.trim().is_empty() {
                    events::append_message(&ctx.meta.id, msg)?;
                }
            }

            for tool_call in tool_calls {
                ctx.log.info(
                    "tool_call",
                    &format!("tool={} id={}", tool_call.name, tool_call.id),
                );
                events::append_tool_call(&ctx.meta.id, std::slice::from_ref(&tool_call))?;

                let content = match tools::execute_tool(
                    &tool_call.name,
                    tool_call.input.clone(),
                    &ctx.working_dir,
                    ctx.allow_scope_escape,
                    &ctx.env_vars,
                ) {
                    Ok(raw) => {
                        ctx.log.info("tool_result", &format!("tool={}", tool_call.name));
                        raw
                    }
                    Err(e) => {
                        ctx.log.error("tool_error", &format!("tool={} err={}", tool_call.name, e));
                        serde_json::Value::String(format!("Error: {}", e))
                    }
                };

                let tool_result = ToolResult {
                    call_id: tool_call.id,
                    content,
                };

                events::append_tool_result(&ctx.meta.id, &tool_result)?;
            }
        } else if let Some(message) = response.message {
            if message.trim().is_empty() {
                ctx.log.warn("empty_response", "");
                let empty_msg = "The previous response contained no text and no tool calls. Please respond with a message or use a tool.".to_string();
                events::append_system(&ctx.meta.id, &empty_msg)?;
            } else {
                ctx.log.info("run_complete", "");
                events::append_message(&ctx.meta.id, &message)?;

                if let Some(ref reasoning) = response.reasoning {
                    events::append_reasoning(&ctx.meta.id, reasoning)?;
                }

                let updates = crate::convo::MetadataUpdate {
                    last_message: Some(message),
                    ..Default::default()
                };
                ctx.store.update(&ctx.meta.id, updates)?;

                break;
            }
        } else {
            ctx.log.warn("empty_response", "");
            let empty_msg = "The previous response contained no text and no tool calls. Please respond with a message or use a tool.".to_string();
            events::append_system(&ctx.meta.id, &empty_msg)?;
        }
    }

    lifecycle::on_run_end(&ctx.meta.id)?;
    guard.disarm();
    ctx.log.info("run_end", &ctx.meta.id);
    Ok(())
}

/// Top-level entry point: setup + loop.
pub fn run(convo_id: &str, provider: &dyn Provider) -> Result<(), String> {
    let mut ctx = build_context(convo_id)?;
    run_loop(&mut ctx, provider)?;
    Ok(())
}

/// Build context with a custom budget (used by tests).
pub fn build_context_with_budget(
    convo_id: &str,
    budget: &TokenBudget,
) -> Result<LoopContext, String> {
    let mut ctx = build_context(convo_id)?;
    ctx.effective_budget = budget.clone();
    Ok(ctx)
}
