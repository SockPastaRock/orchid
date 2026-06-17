use crate::convo::{resolve, MetadataUpdate, Store};
use crate::get_orchid_dir;

pub fn set(
    id: String,
    label: Option<String>,
    persona: Option<String>,
    working_dir: Option<String>,
) -> Result<serde_json::Value, String> {
    let store = Store::new()?;
    let base_path = get_orchid_dir()?.join("conversations");
    let resolved_id = resolve::resolve(&id, &base_path)?.id;

    let mut updates = MetadataUpdate::default();

    if let Some(l) = label {
        updates.label = Some(Some(l));
    }
    if let Some(p) = persona {
        updates.persona = Some(Some(p));
    }
    if let Some(wd) = working_dir {
        updates.working_dir = Some(Some(wd));
    }

    let updated = store.update(&resolved_id, updates)?;
    serde_json::to_value(&updated).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use crate::convo::Store;
    use crate::TestEnv;

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path().to_path_buf();
        let config = serde_json::json!({
            "active_profile": "test",
            "profiles": {"test": {"provider": "anthropic", "api_key": "x", "model": "m"}}
        });
        std::fs::write(dir.join("config.json"), config.to_string()).unwrap();
        (temp, dir)
    }

    #[test]
    fn test_set_label() {
        let (temp, orchid_dir) = setup();
        let _env = TestEnv::with_dir(temp);

        let store = Store::with_base(orchid_dir.join("conversations"));
        let meta = store.create(None, None, None, None).unwrap();

        let result = super::set(meta.id.clone(), Some("my-label".to_string()), None, None).unwrap();
        assert_eq!(result["label"], "my-label");
        assert_eq!(result["id"], meta.id);
    }

    #[test]
    fn test_set_updates_metadata() {
        let (temp, orchid_dir) = setup();
        let _env = TestEnv::with_dir(temp);

        let store = Store::with_base(orchid_dir.join("conversations"));
        let meta = store.create(None, None, None, None).unwrap();

        super::set(
            meta.id.clone(),
            Some("labeled".to_string()),
            Some("coder".to_string()),
            Some("/tmp/work".to_string()),
        )
        .unwrap();

        let updated = store.get(&meta.id).unwrap();
        assert_eq!(updated.label.as_deref(), Some("labeled"));
        assert_eq!(updated.persona.as_deref(), Some("coder"));
        assert_eq!(updated.working_dir.as_deref(), Some("/tmp/work"));
    }
}
