use crate::{get_orchid_dir, load_config, Config, Limits};
use serde_json::json;
use std::fs;

pub fn config_use(profile: &str) -> Result<serde_json::Value, String> {
    let config_dir = get_orchid_dir()?;

    let config_path = config_dir.join("config.json");

    let mut config: Config = if config_path.exists() {
        let contents = fs::read_to_string(&config_path)
            .map_err(|e| format!("failed to read config: {}", e))?;
        serde_json::from_str(&contents).map_err(|e| format!("invalid config JSON: {}", e))?
    } else {
        Config {
            current_profile: None,
            profiles: std::collections::HashMap::new(),
            limits: Limits::default(),
            log_level: None,
            env_file: None,
            extra: std::collections::HashMap::new(),
        }
    };

    if !config.profiles.contains_key(profile) {
        return Err(format!("profile '{}' not found in config", profile));
    }

    config.current_profile = Some(profile.to_string());

    fs::create_dir_all(&config_dir).map_err(|e| format!("failed to create config dir: {}", e))?;

    let temp_path = config_dir.join(".config.json.tmp");
    let json_str = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("failed to serialize config: {}", e))?;

    fs::write(&temp_path, &json_str).map_err(|e| format!("failed to write temp config: {}", e))?;

    fs::rename(&temp_path, &config_path).map_err(|e| format!("failed to update config: {}", e))?;

    Ok(json!({"profile": profile}))
}

pub fn config_current() -> Result<serde_json::Value, String> {
    let config = load_config()?;

    let profile = config
        .current_profile
        .ok_or_else(|| "no profile currently set".to_string())?;

    Ok(json!({"current_profile": profile}))
}

pub fn config_path() -> Result<serde_json::Value, String> {
    let path = get_orchid_dir()?.join("config.json");

    Ok(json!({"path": path.display().to_string()}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path_ok() {
        let result = config_path();
        assert!(result.is_ok());
        let val = result.unwrap();
        assert!(val.get("path").is_some());
        assert!(val["path"].as_str().unwrap().ends_with("config.json"));
    }

    #[test]
    fn test_config_current_missing() {
        // May succeed or fail depending on whether a config exists — just don't panic.
        let _result = config_current();
    }
}
