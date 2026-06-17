use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    // `name` is the map key, not a field in the JSON object — make optional
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub provider: String,
    // Flexible: api_key, base_url, model etc. are profile-specific but unused here
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// Reasoning effort level (e.g., "none", "low", "high").
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    /// Arbitrary headers injected into every request. Values support `env.<VAR>` indirection.
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(flatten, default)]
    pub extra: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Per-session resource limits. All fields optional; unset values use hardcoded defaults.
///
/// Example config.json:
/// ```json
/// {
///   "limits": {
///     "token_warn_threshold": 80000,
///     "token_hard_limit": 120000
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Limits {
    /// Token count at which a warning system message is injected. Default: 80,000.
    pub token_warn_threshold: Option<u32>,
    /// Token count at which the run is terminated. Default: 120,000.
    pub token_hard_limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // Support both "current_profile" (old schema) and "active_profile" (current schema)
    #[serde(alias = "active_profile")]
    pub current_profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
    #[serde(default)]
    pub limits: Limits,
    /// Diagnostic log verbosity: "debug" enables debug-level events in orchid.log.
    /// Omit or set to "info" for default behaviour.
    pub log_level: Option<String>,
    /// Path to a shell env file to source into bash tool executions (e.g. `~/.config/orchid/env`).
    pub env_file: Option<String>,
    // Ignore extra top-level keys (personas, etc.)
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Resolve orchid config directory with XDG standard support.
///
/// Priority (in order):
/// 1. ORCHID_DIR env var (explicit override)
/// 2. XDG_CONFIG_HOME env var (user XDG preference)
/// 3. $HOME/.config (XDG standard)
/// 4. dirs::config_dir().join("orchid") (platform-specific fallback)
pub fn get_orchid_dir() -> Result<PathBuf, String> {
    if let Ok(orchid_dir) = env::var("ORCHID_DIR") {
        return Ok(PathBuf::from(orchid_dir));
    }

    if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg_config_home).join("orchid"));
    }

    if let Ok(home) = env::var("HOME") {
        return Ok(PathBuf::from(home).join(".config").join("orchid"));
    }

    dirs::config_dir()
        .map(|p| p.join("orchid"))
        .ok_or_else(|| "could not determine config directory".to_string())
}

pub fn load_config() -> Result<Config, String> {
    let config_path = config_path()?;

    if !config_path.exists() {
        return Err(format!("config not found at {}", config_path.display()));
    }

    let contents =
        fs::read_to_string(&config_path).map_err(|e| format!("failed to read config: {}", e))?;

    serde_json::from_str(&contents).map_err(|e| format!("invalid config JSON: {}", e))
}

fn config_path() -> Result<PathBuf, String> {
    get_orchid_dir().map(|p| p.join("config.json"))
}

/// Parse a shell env file into key-value pairs.
/// Supports `KEY=VALUE` and `export KEY=VALUE`. Ignores blank lines and comments.
pub fn load_env_file(path: &str) -> HashMap<String, String> {
    let expanded = path.replacen('~', &env::var("HOME").unwrap_or_default(), 1);
    let contents = match fs::read_to_string(&expanded) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let mut vars = HashMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        // Strip optional surrounding quotes from value
        if let Some((key, val)) = line.split_once('=') {
            let val = val.trim_matches('"').trim_matches('\'');
            vars.insert(key.trim().to_string(), val.to_string());
        }
    }
    vars
}

pub fn resolve_env(profile: &Profile) -> HashMap<String, String> {
    let mut resolved = HashMap::new();

    for (key, value) in &profile.env {
        let resolved_value = if let Some(env_var) = value.strip_prefix("env.") {
            env::var(env_var).unwrap_or_else(|_| String::new())
        } else {
            value.clone()
        };

        resolved.insert(key.clone(), resolved_value);
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_env() {
        let mut env_vars = HashMap::new();
        env_vars.insert("API_KEY".to_string(), "env.TEST_VAR".to_string());
        env_vars.insert("LITERAL".to_string(), "value".to_string());

        env::set_var("TEST_VAR", "secret123");

        let profile = Profile {
            name: "test".to_string(),
            provider: "anthropic".to_string(),
            api_key: String::new(),
            base_url: String::new(),
            model: String::new(),
            max_tokens: None,
            reasoning_effort: None,
            extra: std::collections::HashMap::new(),
            headers: std::collections::HashMap::new(),
            env: env_vars,
        };

        let resolved = resolve_env(&profile);

        assert_eq!(
            resolved.get("API_KEY").map(|s| s.as_str()),
            Some("secret123")
        );
        assert_eq!(resolved.get("LITERAL").map(|s| s.as_str()), Some("value"));
    }

    #[test]
    fn test_get_orchid_dir_orchid_dir_override() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .expect("TEST_ENV_LOCK poisoned - a prior test panicked. Check test order.");
        env::set_var("ORCHID_DIR", "/tmp/orchid-test");
        env::remove_var("XDG_CONFIG_HOME");
        env::remove_var("HOME");

        let result = get_orchid_dir().unwrap();
        assert_eq!(result, PathBuf::from("/tmp/orchid-test"));
    }

    #[test]
    fn test_get_orchid_dir_xdg_config_home() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .expect("TEST_ENV_LOCK poisoned - a prior test panicked. Check test order.");
        env::remove_var("ORCHID_DIR");
        env::set_var("XDG_CONFIG_HOME", "/tmp/xdg");
        env::remove_var("HOME");

        let result = get_orchid_dir().unwrap();
        assert_eq!(result, PathBuf::from("/tmp/xdg/orchid"));
    }

    #[test]
    fn test_get_orchid_dir_home_fallback() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .expect("TEST_ENV_LOCK poisoned - a prior test panicked. Check test order.");
        env::remove_var("ORCHID_DIR");
        env::remove_var("XDG_CONFIG_HOME");
        env::set_var("HOME", "/home/user");

        let result = get_orchid_dir().unwrap();
        assert_eq!(result, PathBuf::from("/home/user/.config/orchid"));
    }
}
