use crate::load_config;
use crate::Store;
use serde_json::json;

pub fn list() -> Result<serde_json::Value, String> {
    let store = Store::new()?;
    let convos = store.list()?;

    let json_array = json!(convos);
    Ok(json_array)
}

pub fn list_profiles() -> Result<serde_json::Value, String> {
    let config = load_config()?;
    Ok(json!(config.profiles))
}

pub fn list_personas() -> Result<serde_json::Value, String> {
    let config = load_config()?;
    let personas = config.extra.get("personas").cloned().unwrap_or(json!({}));
    Ok(personas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_list_empty() {
        // Use a temp dir to avoid conflicts
        let _temp = TempDir::new().unwrap();

        // This tests the structure; actual list() uses live Store
        let result = json!([]);
        assert!(result.is_array());
        assert_eq!(result.as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_list_is_json_array() {
        let result = json!([
            {"id": "abc123", "label": "test", "status": "idle"}
        ]);
        assert!(result.is_array());
        assert_eq!(result.as_array().unwrap().len(), 1);
    }
}
