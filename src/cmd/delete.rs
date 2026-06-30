use crate::convo::resolve;
use crate::get_orchid_dir;
use serde_json::json;
use std::fs;

pub fn delete(id: String) -> Result<serde_json::Value, String> {
    let orchid_dir = get_orchid_dir()?;
    let base_path = orchid_dir.join("conversations");
    let resolved_id = resolve::resolve(&id, &base_path)?.id;
    let convo_path = base_path.join(&resolved_id);

    if !convo_path.exists() {
        return Err(format!("conversation '{}' not found", id));
    }

    let archive_base = base_path.join(".archive");

    fs::create_dir_all(&archive_base)
        .map_err(|e| format!("failed to create archive dir: {}", e))?;

    let archive_path = archive_base.join(&resolved_id);

    fs::rename(&convo_path, &archive_path)
        .map_err(|e| format!("failed to move conversation to archive: {}", e))?;

    Ok(json!({
        "id": resolved_id,
        "status": "archived",
        "archived_at": chrono::Utc::now().to_rfc3339()
    }))
}

#[cfg(test)]
mod tests {
    use crate::convo::Store;
    use crate::TestEnv;

    #[test]
    fn test_delete_not_found() {
        let _env = TestEnv::new();

        let fake_id = "a".repeat(32);
        let err = super::delete(fake_id).unwrap_err();
        assert!(
            err.contains("not found") || err.contains("conversation not found"),
            "got: {}",
            err
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_delete_creates_archive() {
        let env = TestEnv::new();
        let orchid_dir = env.dir();

        let convos_dir = orchid_dir.join("conversations");
        std::fs::create_dir_all(&convos_dir).unwrap();
        let store = Store::with_base(convos_dir.clone());
        let meta = store.create(None, None, None, None).unwrap();

        assert!(convos_dir.join(&meta.id).exists());

        let result = super::delete(meta.id.clone()).unwrap();
        assert_eq!(result["id"], meta.id);
        assert_eq!(result["status"], "archived");

        assert!(
            !convos_dir.join(&meta.id).exists(),
            "conversation dir should be gone after archive"
        );
        assert!(
            convos_dir.join(".archive").join(&meta.id).exists(),
            "conversation should be in .archive"
        );
    }
}
