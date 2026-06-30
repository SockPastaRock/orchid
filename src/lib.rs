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
    pub use crate::r#loop::run::run;
}

pub use cli::{parse_args, Command, ConfigSubcommand};
pub use client::{create_provider, create_provider_with_log};
pub use cmd::{config_current, config_path, config_use, delete, internal_run, list, send, set};
pub use config::{get_orchid_dir, load_config, resolve_env, Config, Limits, Profile, ServerAction};
pub use convo::{get_convo_jsonl_path, MetadataUpdate, Store};
pub use jsonerr::JsonError;
pub use log::{DiagLogger, LogReader, LogWriter};
pub use provider::{Provider, ProviderError, Response, StreamEvent};
pub use tools::{execute_tool, tool_definitions, Tool};
pub use types::{Message, Metadata, Status, TokenBudget, ToolCall, ToolResult};

#[cfg(test)]
use std::env;

/// A guard that creates a unique temp directory, sets `ORCHID_DIR` to it,
/// and restores the previous value on drop.
///
/// Tests simply call `TestEnv::new()` — no need to create a separate `TempDir`.
///
/// **Important:** Tests share global state (`ORCHID_DIR` env var), so they must
/// run sequentially. Use `make test` (which passes `--test-threads=1`) or
/// `cargo test -- --test-threads=1`. Running tests in parallel will cause
/// flaky failures as tests clobber each other's temp directories.
#[cfg(test)]
pub(crate) struct TestEnv {
    prev: Option<String>,
    temp: tempfile::TempDir,
}

#[cfg(test)]
impl TestEnv {
    /// Create a new test environment: generates a unique temp directory,
    /// sets `ORCHID_DIR` to it, and restores the previous value on drop.
    pub(crate) fn new() -> Self {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path().to_path_buf();
        let prev = env::var("ORCHID_DIR").ok();
        env::set_var("ORCHID_DIR", dir.to_string_lossy().to_string());
        Self {
            prev,
            temp,
        }
    }

    /// Return the path to the test's temp directory.
    pub(crate) fn dir(&self) -> std::path::PathBuf {
        self.temp.path().to_path_buf()
    }
}

#[cfg(test)]
impl Drop for TestEnv {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => env::set_var("ORCHID_DIR", v.clone()),
            None => env::remove_var("ORCHID_DIR"),
        }
    }
}
