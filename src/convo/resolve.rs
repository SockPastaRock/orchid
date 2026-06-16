use crate::types::Metadata;
use std::path::Path;

pub fn resolve(id: &str, base_path: &Path) -> Result<Metadata, String> {
    if !is_id_format(id) {
        return Err(format!(
            "invalid conversation ID: '{}' (must be 32 hex characters)",
            id
        ));
    }
    read_metadata(id, base_path)
}

fn is_id_format(s: &str) -> bool {
    s.len() == 32 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn read_metadata(id: &str, base_path: &Path) -> Result<Metadata, String> {
    let metadata_path = base_path.join(id).join("metadata.json");
    let contents = std::fs::read_to_string(&metadata_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("conversation not found: {}", id)
        } else {
            format!("failed to read metadata: {}", e)
        }
    })?;
    serde_json::from_str(&contents).map_err(|e| format!("invalid metadata JSON: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_id_format() {
        assert!(is_id_format("abcdef0123456789abcdef0123456789"));
        assert!(!is_id_format("short"));
        assert!(!is_id_format("not_hex_!@#$%abcdef0123456789abcdef01234"));
    }

    #[test]
    fn test_resolve_rejects_non_id() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let err = resolve("my-label", temp.path()).unwrap_err();
        assert!(err.contains("invalid conversation ID"), "got: {}", err);
    }
}
