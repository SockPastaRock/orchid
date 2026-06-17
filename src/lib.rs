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
pub use config::{get_orchid_dir, load_config, resolve_env, Config, Limits, Profile};
pub use convo::{get_convo_jsonl_path, MetadataUpdate, Store};
pub use jsonerr::JsonError;
pub use log::{DiagLogger, LogReader, LogWriter};
pub use provider::{Provider, ProviderError, Response, StreamEvent};
pub use tools::{execute_tool, tool_definitions, Tool};
pub use types::{Message, Metadata, Status, TokenBudget, ToolCall, ToolResult};

use std::env;
use std::sync::OnceLock;

static NEXT_TEST_ID: OnceLock<std::sync::atomic::AtomicU64> = OnceLock::new();
fn get_test_id() -> u64 {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

// Per-test mutexes: each test gets its own mutex to serialize its own ORCHID_DIR reads.
static TEST_MUTEXES: OnceLock<std::sync::Mutex<Vec<std::sync::Mutex<()>>>> = OnceLock::new();
fn get_test_mutex(id: u64) -> std::sync::MutexGuard<'static, ()> {
    let mutexes = TEST_MUTEXES.get_or_init(|| {
        std::sync::Mutex::new((0..1024).map(|_| std::sync::Mutex::new(())).collect())
    });
    mutexes.lock().unwrap()[id as usize].lock().unwrap()
}

/// A guard that temporarily sets `ORCHID_DIR` and restores the previous value on drop.
///
/// This enables order-independent, parallel-safe tests by ensuring each test's
/// `ORCHID_DIR` setting is automatically reverted when the guard goes out of scope.
#[cfg(test)]
pub(crate) struct TestEnv {
    prev: Option<String>,
    _temp: Option<tempfile::TempDir>,
}

#[cfg(test)]
impl TestEnv {
    /// Set `ORCHID_DIR` to the path of `temp` and save the previous value (if any).
    /// The `TempDir` is kept alive for the duration of the guard.
    pub(crate) fn with_dir(temp: tempfile::TempDir) -> Self {
        let dir = temp.path().to_path_buf();
        let prev = env::var("ORCHID_DIR").ok();
        env::set_var("ORCHID_DIR", dir.to_string_lossy().to_string());
        Self {
            prev,
            _temp: Some(temp),
        }
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
