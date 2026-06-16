pub mod cli;
pub mod client;
pub mod cmd;
pub mod config;
pub mod convo;
pub mod jsonerr;
pub mod log;
pub mod r#loop;
pub mod provider;
pub mod tools;
pub mod types;
pub mod loop_module {
    pub use crate::r#loop::run;
}

pub use cli::{parse_args, Command, ConfigSubcommand};
pub use client::{create_provider, create_provider_with_log};
pub use cmd::{config_current, config_path, config_use, delete, internal_run, list, send, set};
pub use config::{get_orchid_dir, load_config, resolve_env, Config, Limits, Profile};
pub use convo::{get_convo_jsonl_path, MetadataUpdate, Store};
pub use jsonerr::JsonError;
pub use log::{DiagLogger, LogReader, LogWriter};
pub use provider::{Provider, ProviderError, Response, StreamEvent};
pub use tools::{execute_tool, tool_definitions, Tool};
pub use types::{Message, Metadata, Status, TokenBudget, ToolCall, ToolResult};

/// Crate-wide mutex for tests that mutate the ORCHID_DIR environment variable.
/// ORCHID_DIR is process-global state; tests that set it must serialise via this
/// lock to avoid races with other tests reading it concurrently.
#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
