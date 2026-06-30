use crate::{create_provider, load_config, r#loop};

pub fn internal_run(convo_id: &str, profile: &Option<String>) -> Result<(), String> {
    let config = load_config()?;

    let profile_name = profile.clone().unwrap_or(
        config
            .current_profile
            .ok_or_else(|| "no profile configured".to_string())?,
    );

    let profiles = config.profiles;
    let prof = profiles
        .get(&profile_name)
        .ok_or_else(|| format!("profile '{}' not found", profile_name))?;

    let provider = create_provider(prof).map_err(|e| format!("provider error: {}", e))?;

    r#loop::run(convo_id, provider.as_ref())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::TestEnv;

    #[test]
    fn test_internal_run_unknown_profile() {
        let env = TestEnv::new();
        let orchid_dir = env.dir();
        let config = serde_json::json!({
            "active_profile": "default",
            "profiles": {"default": {"provider": "anthropic", "api_key": "x", "model": "m"}}
        });
        std::fs::write(orchid_dir.join("config.json"), config.to_string()).unwrap();

        let err = super::internal_run("nonexistent_id", &Some("missing-profile".to_string()))
            .unwrap_err();
        assert!(
            err.contains("not found") || err.contains("profile"),
            "got: {}",
            err
        );
    }
}
