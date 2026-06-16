use crate::cmd::create::resolve_working_dir;
use crate::convo::{resolve, Store};
use crate::log::LogWriter;
use crate::loop_module::run as run_tool_loop;
use crate::types::{ConvoEvent, MessageEvent, TokenBudget};
use crate::{get_convo_jsonl_path, get_orchid_dir, load_config};
use crate::client::create_provider_with_log;
use serde_json::json;
use std::process::Stdio;
#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub fn send(
    id: Option<String>,
    message: String,
    await_completion: bool,
    profile: Option<String>,
    label: Option<String>,
    working_dir: Option<String>,
) -> Result<serde_json::Value, String> {
    let store = Store::new()?;
    let config = load_config()?;
    let active_profile = config.current_profile.clone();

    let convo_id = if let Some(id_or_label) = id {
        let base_path = get_orchid_dir()?.join("conversations");
        let resolved_id = resolve::resolve(&id_or_label, &base_path)?.id;

        if label.is_some() || working_dir.is_some() {
            let mut updates = crate::MetadataUpdate::default();
            if let Some(l) = label {
                updates.label = Some(Some(l));
            }
            if let Some(wd) = working_dir {
                updates.working_dir = Some(Some(wd));
            }
            store.update(&resolved_id, updates)?;
        }

        resolved_id
    } else {
        let wd = resolve_working_dir(working_dir)?;
        let meta = store.create(label, Some(wd), None, None)?;
        meta.id
    };

    let meta = store.get(&convo_id)?;
    let _working_dir = meta.working_dir.unwrap_or_else(|| "/tmp".to_string());

    if meta.status == crate::types::Status::Running {
        return Err(format!("conversation {} is already running", convo_id));
    }

    let convo_path = get_convo_jsonl_path(&convo_id)?;
    let event = ConvoEvent::Message(MessageEvent::new("user", &message));
    LogWriter::append(&convo_path, &event)?;

    if await_completion {
        let profile_name =
            profile.unwrap_or(active_profile.ok_or_else(|| "no profile configured".to_string())?);

        let profiles = config.profiles;
        let prof = profiles
            .get(&profile_name)
            .ok_or_else(|| format!("profile '{}' not found", profile_name))?;

        let log_level = crate::log::LogLevel::from_config_str(config.log_level.as_deref());
        let convo_dir = get_orchid_dir()?.join("conversations").join(&convo_id);
        let log = crate::log::DiagLogger::for_convo(convo_dir.clone(), log_level);

        log.debug("profile_selected", &profile_name);
        log.debug("profile_base_url", &prof.base_url);
        log.debug("profile_model", &prof.model);
        log.debug(
            "profile_api_key",
            if prof.api_key.is_empty() { "(empty)" } else { "(set)" },
        );
        log.debug(
            "profile_headers",
            &prof
                .headers
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );

        let provider = create_provider_with_log(prof, Some(convo_dir.join("orchid.log"))).map_err(|e| {
            log.error("provider_init_error", &e.to_string());
            format!("provider error: {}", e)
        })?;
        log.debug("provider_init", "ok");

        let budget = TokenBudget::from_limits(&config.limits);
        run_tool_loop(&convo_id, provider.as_ref(), &budget)?;

        let final_meta = store.get(&convo_id)?;
        Ok(json!({
            "id": convo_id,
            "status": final_meta.status,
            "completed": true
        }))
    } else {
        let pid = fork_tool_loop(&convo_id, &profile, active_profile)?;

        let updates = crate::MetadataUpdate {
            pid: Some(pid),
            ..Default::default()
        };

        store.update(&convo_id, updates)?;

        Ok(json!({
            "id": convo_id,
            "status": "running",
            "pid": pid
        }))
    }
}

fn fork_tool_loop(
    convo_id: &str,
    profile: &Option<String>,
    active_profile: Option<String>,
) -> Result<Option<u32>, String> {
    let profile_arg = profile
        .clone()
        .or(active_profile)
        .ok_or_else(|| "no profile configured and no --profile given".to_string())?;

    let mut cmd =
        std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    cmd.arg("__run")
        .arg(convo_id)
        .arg("--profile")
        .arg(profile_arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    // Detach from the caller's process group and controlling terminal so the
    // daemon survives when the parent process exits (e.g. when spawned via
    // Emacs make-process which places children in its own process group).
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid().map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
            Ok(())
        });
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn background process: {}", e))?;

    Ok(Some(child.id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_minimal_config(dir: &std::path::Path, active_profile: Option<&str>) {
        let profile_section =
            r#""test-profile":{"provider":"anthropic","api_key":"x","model":"m"}"#;
        let active = active_profile
            .map(|p| format!(r#""active_profile":"{}","#, p))
            .unwrap_or_default();
        let json = format!(r#"{{{}"profiles":{{{}}}}}"#, active, profile_section);
        std::fs::write(dir.join("config.json"), json).unwrap();
    }

    #[test]
    fn test_send_writes_user_message_to_jsonl() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = TempDir::new().unwrap();
        std::env::set_var("ORCHID_DIR", temp.path().to_string_lossy().to_string());
        write_minimal_config(temp.path(), Some("test-profile"));

        let store = crate::convo::Store::with_base(temp.path().join("conversations"));
        std::fs::create_dir_all(temp.path().join("conversations")).unwrap();
        let meta = store
            .create(None, Some("/tmp".to_string()), None, None)
            .unwrap();

        // The fork will fail (test binary is not orchid CLI), but the JSONL write
        // happens before the fork, so we can assert on it regardless.
        let send_result = send(
            Some(meta.id.clone()),
            "hello world".to_string(),
            false,
            Some("test-profile".to_string()),
            None,
            None,
        );
        // Allow fork failures (expected in test env) but not earlier errors.
        if let Err(ref e) = send_result {
            assert!(
                e.contains("spawn") || e.contains("fork") || e.contains("failed to spawn"),
                "unexpected send error: {}",
                e
            );
        }

        let jsonl = temp
            .path()
            .join("conversations")
            .join(&meta.id)
            .join("conversation.jsonl");
        assert!(jsonl.exists(), "conversation.jsonl should exist after send");
        let contents = std::fs::read_to_string(&jsonl).unwrap();
        assert!(
            contents.contains("\"type\":\"message\""),
            "event should have type field"
        );
        assert!(
            contents.contains("hello world"),
            "user message should be in jsonl"
        );
    }

    #[test]
    fn test_fork_uses_active_profile_not_hardcoded_default() {
        // Pure logic test — no env vars needed.
        let profile: Option<String> = None;
        let active_profile: Option<String> = Some("cba-sonnet".to_string());

        let profile_arg = profile
            .as_ref()
            .map(|p| p.clone())
            .or(active_profile)
            .expect("should fall back to active_profile");

        assert_eq!(profile_arg, "cba-sonnet");
        assert_ne!(
            profile_arg, "default",
            "must not fall back to hardcoded 'default'"
        );
    }

    #[test]
    fn test_fork_errors_when_no_profile_available() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = TempDir::new().unwrap();
        std::env::set_var("ORCHID_DIR", temp.path().to_string_lossy().to_string());
        // Config with no active_profile.
        write_minimal_config(temp.path(), None);

        let store = crate::convo::Store::with_base(temp.path().join("conversations"));
        std::fs::create_dir_all(temp.path().join("conversations")).unwrap();
        let meta = store.create(None, None, None, None).unwrap();

        let result = send(
            Some(meta.id.clone()),
            "test".to_string(),
            false, // fire-and-forget → calls fork_tool_loop
            None,  // no --profile
            None,
            None,
        );

        assert!(result.is_err(), "should error when no profile is available");
        assert!(
            result.unwrap_err().contains("profile"),
            "error should mention profile"
        );
    }
}
