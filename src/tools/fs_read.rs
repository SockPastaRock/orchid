use crate::tools::scope::is_in_scope;
use serde_json::Value;
use std::fs;

/// Extract paths from an fs_read tool call input.
/// Supports both `{"paths": [...]}` (batch) and legacy `{"path": "..."}`.
pub fn extract_paths(input: &Value) -> Vec<String> {
    if let Some(paths) = input.get("paths").and_then(|v| v.as_array()) {
        paths
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    } else if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
        vec![path.to_string()]
    } else {
        vec![]
    }
}

/// Returns a JSON object: `{"<path>": "<content>", ...}`.
/// Errors in individual files are represented as `{"error": "<msg>"}` values.
/// A single-path read that fails propagates the error directly.
pub fn execute(input: Value, working_dir: &str, allow_scope_escape: bool) -> Result<Value, String> {
    let paths = extract_paths(&input);

    if paths.is_empty() {
        return Err("invalid fs_read input: expected 'paths' array or 'path' string".to_string());
    }

    if paths.len() == 1 {
        let content = read_one(&paths[0], working_dir, allow_scope_escape)?;
        Ok(serde_json::json!({ &paths[0]: content }))
    } else {
        let mut map = serde_json::Map::new();
        for path in &paths {
            match read_one(path, working_dir, allow_scope_escape) {
                Ok(content) => {
                    map.insert(path.clone(), Value::String(content));
                }
                Err(e) => {
                    map.insert(path.clone(), serde_json::json!({"error": e}));
                }
            }
        }
        Ok(Value::Object(map))
    }
}

fn read_one(path: &str, working_dir: &str, allow_scope_escape: bool) -> Result<String, String> {
    if !allow_scope_escape && !is_in_scope(path, working_dir) {
        return Err(format!("path out of scope: {}", path));
    }

    let resolved = crate::tools::scope::expand_path(path, working_dir);
    fs::read_to_string(&resolved).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("file not found: {}", path)
        } else if e.kind() == std::io::ErrorKind::PermissionDenied {
            format!("permission denied: {}", path)
        } else {
            format!("failed to read file: {}", e)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_read_single_returns_json_object() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "test content").unwrap();
        drop(file);

        let path = file_path.to_string_lossy().to_string();
        let work_dir = temp_dir.path().to_string_lossy().to_string();

        let result = execute(serde_json::json!({"path": path.clone()}), &work_dir, false).unwrap();
        assert!(result[&path].as_str().unwrap().contains("test content"));
    }

    #[test]
    fn test_read_batch_returns_json_object() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let work_dir = temp_dir.path().to_string_lossy().to_string();

        for name in ["a.txt", "b.txt"] {
            let mut f = std::fs::File::create(temp_dir.path().join(name)).unwrap();
            writeln!(f, "content of {}", name).unwrap();
        }

        let pa = temp_dir.path().join("a.txt").to_string_lossy().to_string();
        let pb = temp_dir.path().join("b.txt").to_string_lossy().to_string();

        let result = execute(
            serde_json::json!({"paths": [pa.clone(), pb.clone()]}),
            &work_dir,
            false,
        )
        .unwrap();
        assert!(result[&pa].as_str().unwrap().contains("content of a.txt"));
        assert!(result[&pb].as_str().unwrap().contains("content of b.txt"));
    }

    #[test]
    fn test_read_batch_partial_error_is_structured() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let work_dir = temp_dir.path().to_string_lossy().to_string();

        let pa = temp_dir
            .path()
            .join("exists.txt")
            .to_string_lossy()
            .to_string();
        let mut f = std::fs::File::create(&pa).unwrap();
        writeln!(f, "hello").unwrap();
        drop(f);

        let pb = temp_dir
            .path()
            .join("missing.txt")
            .to_string_lossy()
            .to_string();

        let result = execute(
            serde_json::json!({"paths": [pa.clone(), pb.clone()]}),
            &work_dir,
            false,
        )
        .unwrap();
        assert!(result[&pa].as_str().unwrap().contains("hello"));
        assert!(
            result[&pb]["error"].as_str().is_some(),
            "missing file error should be an object with 'error' key"
        );
    }

    #[test]
    fn test_read_nonexistent_single_propagates_error() {
        let result = execute(
            serde_json::json!({"path": "/tmp/nonexistent_file_xyz"}),
            "/tmp",
            false,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("file not found"));
    }

    #[test]
    fn test_read_out_of_scope() {
        let result = execute(serde_json::json!({"path": "/etc/passwd"}), "/tmp", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of scope"));
    }

    #[test]
    fn test_extract_paths_paths_key() {
        let v = serde_json::json!({"paths": ["a.rs", "b.rs"]});
        assert_eq!(extract_paths(&v), vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn test_extract_paths_path_key() {
        let v = serde_json::json!({"path": "a.rs"});
        assert_eq!(extract_paths(&v), vec!["a.rs"]);
    }
}
