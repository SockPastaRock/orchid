use crate::convo::{resolve, Store};
use crate::types::Status;
use serde_json::json;

pub fn stop(id: String) -> Result<serde_json::Value, String> {
    stop_impl(&id, false)
}

fn stop_impl(id: &str, force: bool) -> Result<serde_json::Value, String> {
    let store = Store::new()?;
    let base_path = crate::get_orchid_dir()?.join("conversations");
    let meta = resolve::resolve(id, &base_path)?;
    let convo_id = meta.id;

    if meta.status == Status::Idle {
        return Ok(json!({
            "id": convo_id,
            "status": "idle",
            "message": "conversation is not running"
        }));
    }

    if let Some(pid) = meta.pid {
        #[cfg(unix)]
        {
            use nix::sys::signal::{self, Signal};
            use nix::unistd::Pid;

            let pid = Pid::from_raw(pid as i32);
            let sig = if force { Signal::SIGKILL } else { Signal::SIGTERM };
            match signal::kill(pid, Some(sig)) {
                Ok(()) => {}
                Err(nix::Error::ESRCH) => {}
                Err(e) => return Err(format!("failed to send signal: {}", e)),
            }
        }

        #[cfg(not(unix))]
        let _ = pid;
    }

    #[cfg(unix)]
    if !force {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    store.update(
        &convo_id,
        crate::convo::MetadataUpdate {
            status: Some(Status::Idle),
            pid: Some(None),
            run_started_at: Some(None),
            ..Default::default()
        },
    )?;

    Ok(json!({
        "id": convo_id,
        "status": "stopped",
        "killed": true
    }))
}
