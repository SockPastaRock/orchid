use crate::get_orchid_dir;
use crate::types::{Metadata, Status};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;

pub mod id;
pub mod resolve;

pub struct Store {
    base_path: PathBuf,
}

impl Store {
    pub fn new() -> Result<Self, String> {
        let base_path = get_orchid_dir()?.join("conversations");

        fs::create_dir_all(&base_path)
            .map_err(|e| format!("failed to create conversations dir: {}", e))?;

        Ok(Store { base_path })
    }

    #[cfg(test)]
    pub fn with_base(base_path: PathBuf) -> Self {
        Store { base_path }
    }

    pub fn create(
        &self,
        label: Option<String>,
        working_dir: Option<String>,
        persona: Option<String>,
        _profile: Option<String>,
    ) -> Result<Metadata, String> {
        loop {
            let id = id::generate_id();

            if !id::exists_check(&id, &self.base_path) {
                let convo_dir = self.base_path.join(&id);
                fs::create_dir_all(&convo_dir)
                    .map_err(|e| format!("failed to create conversation dir: {}", e))?;

                let now = Utc::now();
                let meta = Metadata {
                    id: id.clone(),
                    label,
                    persona,
                    working_dir,
                    env: None,
                    created_at: now,
                    updated_at: now,
                    status: Status::Idle,
                    pid: None,
                    run_started_at: None,
                    last_run_at: None,
                    last_message: None,
                    hooks: None,
                    token_estimate: None,
                    allow_scope_escape: None,
                };

                self.write_metadata(&id, &meta)?;
                return Ok(meta);
            }
        }
    }

    pub fn get(&self, id: &str) -> Result<Metadata, String> {
        let metadata_path = self.base_path.join(id).join("metadata.json");
        let contents = fs::read_to_string(&metadata_path)
            .map_err(|e| format!("failed to read metadata: {}", e))?;
        serde_json::from_str(&contents).map_err(|e| format!("invalid metadata JSON: {}", e))
    }

    pub fn list(&self) -> Result<Vec<Metadata>, String> {
        let mut convos = Vec::new();
        let entries = fs::read_dir(&self.base_path)
            .map_err(|e| format!("failed to read conversations dir: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("dir entry error: {}", e))?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(meta) = self.get(dir_name) {
                        convos.push(meta);
                    }
                }
            }
        }

        convos.sort_by(|a, b| {
            let a_time = a.run_started_at.unwrap_or(a.created_at);
            let b_time = b.run_started_at.unwrap_or(b.created_at);
            b_time.cmp(&a_time)
        });

        Ok(convos)
    }

    pub fn update(&self, id: &str, updates: MetadataUpdate) -> Result<Metadata, String> {
        let mut meta = self.get(id)?;

        if let Some(label) = updates.label {
            meta.label = label;
        }
        if let Some(persona) = updates.persona {
            meta.persona = persona;
        }
        if let Some(working_dir) = updates.working_dir {
            meta.working_dir = working_dir;
        }
        if let Some(status) = updates.status {
            meta.status = status;
        }
        if let Some(pid) = updates.pid {
            meta.pid = pid;
        }
        if let Some(run_started_at) = updates.run_started_at {
            meta.run_started_at = run_started_at;
        }
        if let Some(last_run_at) = updates.last_run_at {
            meta.last_run_at = last_run_at;
        }
        if let Some(last_message) = updates.last_message {
            meta.last_message = Some(last_message);
        }
        if let Some(token_estimate) = updates.token_estimate {
            meta.token_estimate = Some(token_estimate);
        }

        meta.updated_at = Utc::now();

        self.write_metadata(id, &meta)?;
        Ok(meta)
    }

    fn write_metadata(&self, id: &str, meta: &Metadata) -> Result<(), String> {
        let metadata_path = self.base_path.join(id).join("metadata.json");
        let temp_path = self.base_path.join(id).join(".metadata.json.tmp");

        let json = serde_json::to_string_pretty(meta)
            .map_err(|e| format!("failed to serialize metadata: {}", e))?;

        fs::write(&temp_path, &json)
            .map_err(|e| format!("failed to write temp metadata: {}", e))?;

        fs::rename(&temp_path, &metadata_path)
            .map_err(|e| format!("failed to rename metadata file: {}", e))?;

        Ok(())
    }
}

#[derive(Default)]
pub struct MetadataUpdate {
    pub label: Option<Option<String>>,
    pub persona: Option<Option<String>>,
    pub working_dir: Option<Option<String>>,
    pub status: Option<Status>,
    pub pid: Option<Option<u32>>,
    pub run_started_at: Option<Option<chrono::DateTime<chrono::Utc>>>,
    pub last_run_at: Option<Option<chrono::DateTime<chrono::Utc>>>,
    pub last_message: Option<String>,
    pub token_estimate: Option<u32>,
}

/// Helper to resolve convo.jsonl path with XDG support.
pub fn get_convo_jsonl_path(convo_id: &str) -> Result<PathBuf, String> {
    let base_path = get_orchid_dir()?.join("conversations").join(convo_id);

    Ok(base_path.join("conversation.jsonl"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_create_conversation() {
        let temp_dir = TempDir::new().unwrap();
        let store = Store {
            base_path: temp_dir.path().to_path_buf(),
        };

        let meta = store
            .create(
                Some("test-conv".to_string()),
                Some("/tmp".to_string()),
                None,
                None,
            )
            .unwrap();

        assert_eq!(meta.label, Some("test-conv".to_string()));
        assert_eq!(meta.working_dir, Some("/tmp".to_string()));
        assert_eq!(meta.status, Status::Idle);
        assert_eq!(meta.id.len(), 32);
    }

    #[test]
    fn test_get_conversation() {
        let temp_dir = TempDir::new().unwrap();
        let store = Store {
            base_path: temp_dir.path().to_path_buf(),
        };

        let created = store
            .create(Some("test-get".to_string()), None, None, None)
            .unwrap();
        let retrieved = store.get(&created.id).unwrap();

        assert_eq!(created.id, retrieved.id);
        assert_eq!(created.label, retrieved.label);
    }

    #[test]
    fn test_list_conversations() {
        let temp_dir = TempDir::new().unwrap();
        let store = Store {
            base_path: temp_dir.path().to_path_buf(),
        };

        store
            .create(Some("first".to_string()), None, None, None)
            .unwrap();
        store
            .create(Some("second".to_string()), None, None, None)
            .unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_update_conversation() {
        let temp_dir = TempDir::new().unwrap();
        let store = Store {
            base_path: temp_dir.path().to_path_buf(),
        };

        let meta = store
            .create(Some("test-update".to_string()), None, None, None)
            .unwrap();

        let mut updates = MetadataUpdate::default();
        updates.status = Some(Status::Running);

        let updated = store.update(&meta.id, updates).unwrap();
        assert_eq!(updated.status, Status::Running);
    }

    #[test]
    fn test_atomic_write() {
        let temp_dir = TempDir::new().unwrap();
        let store = Store {
            base_path: temp_dir.path().to_path_buf(),
        };

        let meta = store.create(None, None, None, None).unwrap();
        let metadata_path = store.base_path.join(&meta.id).join("metadata.json");

        assert!(metadata_path.exists());
        let contents = fs::read_to_string(&metadata_path).unwrap();
        let _: Metadata = serde_json::from_str(&contents).unwrap();
    }
}
