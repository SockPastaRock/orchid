use crate::tools::scope::is_in_scope;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::io::Write;

#[derive(Deserialize)]
pub struct Edit {
    pub old_string: String,
    pub new_string: String,
    #[serde(default)]
    pub replace_all: bool,
}

#[derive(Deserialize)]
pub struct FsEditInput {
    pub path: String,
    /// Batch edits — applied in sequence, fail fast.
    /// Accepts either a JSON array or a JSON-encoded string (model compat).
    #[serde(default, deserialize_with = "deserialize_edits")]
    pub edits: Vec<Edit>,
    /// Legacy single-edit fields (still accepted for backward compatibility).
    pub old_string: Option<String>,
    pub new_string: Option<String>,
    #[serde(default)]
    pub replace_all: bool,
}

fn deserialize_edits<'de, D>(deserializer: D) -> Result<Vec<Edit>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Value::deserialize(deserializer)?;
    match v {
        Value::Array(_) => serde_json::from_value(v).map_err(serde::de::Error::custom),
        Value::String(s) => {
            // Model sent the array as a JSON-encoded string — parse it.
            serde_json::from_str(&s).map_err(serde::de::Error::custom)
        }
        Value::Null => Ok(Vec::new()),
        other => Err(serde::de::Error::custom(format!(
            "expected array or JSON string for edits, got: {}",
            other
        ))),
    }
}

pub fn execute(input: Value, working_dir: &str, allow_scope_escape: bool) -> Result<String, String> {
    let edit_input: FsEditInput =
        serde_json::from_value(input).map_err(|e| format!("invalid fs_edit input: {}", e))?;

    if !allow_scope_escape && !is_in_scope(&edit_input.path, working_dir) {
        return Err(format!("path out of scope: {}", edit_input.path));
    }

    let resolved_path = crate::tools::scope::expand_path(&edit_input.path, working_dir);

    // Build the edits list: prefer `edits` array, fall back to legacy fields.
    let edits: Vec<Edit> = if !edit_input.edits.is_empty() {
        edit_input.edits
    } else if let (Some(old), Some(new)) = (edit_input.old_string, edit_input.new_string) {
        vec![Edit {
            old_string: old,
            new_string: new,
            replace_all: edit_input.replace_all,
        }]
    } else {
        return Err(
            "invalid fs_edit input: provide 'edits' array or 'old_string'/'new_string'".to_string(),
        );
    };

    // Create-file shortcut: single edit with empty old_string.
    if edits.len() == 1 && edits[0].old_string.is_empty() {
        return create_file(&resolved_path, &edits[0].new_string);
    }

    let original = match fs::read_to_string(&resolved_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("file not found: {}", resolved_path));
        }
        Err(e) => return Err(format!("failed to read file: {}", e)),
    };

    // Apply all edits to an in-memory copy; fail fast before touching disk.
    let mut working = original;
    let mut total_replacements = 0usize;

    for (i, edit) in edits.iter().enumerate() {
        let count = working.matches(&edit.old_string).count();
        if count == 0 {
            return Err(format!(
                "edit {}: pattern not found: {}",
                i + 1,
                edit.old_string
            ));
        }
        if count > 1 && !edit.replace_all {
            return Err(format!(
                "edit {}: multiple matches ({}) without replace_all=true",
                i + 1,
                count
            ));
        }
        working = if edit.replace_all {
            working.replace(&edit.old_string, &edit.new_string)
        } else {
            working.replacen(&edit.old_string, &edit.new_string, 1)
        };
        total_replacements += count;
    }

    write_atomic(&resolved_path, &working)?;
    let _ = total_replacements;
    Ok(serde_json::json!({
        "path": resolved_path,
        "edits_applied": edits.len()
    })
    .to_string())
}
fn create_file(path: &str, content: &str) -> Result<String, String> {
    let mut file = fs::File::create(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            format!("permission denied: {}", path)
        } else {
            format!("failed to create file: {}", e)
        }
    })?;

    file.write_all(content.as_bytes())
        .map_err(|e| format!("failed to write file: {}", e))?;

    Ok(serde_json::json!({"path": path, "created": true}).to_string())
}

fn write_atomic(path: &str, content: &str) -> Result<(), String> {
    let temp_path = format!("{}.tmp", path);

    let mut temp_file =
        fs::File::create(&temp_path).map_err(|e| format!("failed to create temp file: {}", e))?;

    temp_file
        .write_all(content.as_bytes())
        .map_err(|e| format!("failed to write temp file: {}", e))?;

    drop(temp_file);

    fs::rename(&temp_path, path).map_err(|e| format!("failed to rename temp file: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_create_new_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("new.txt")
            .to_string_lossy()
            .to_string();
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        let input = serde_json::json!({
            "path": path.clone(),
            "old_string": "",
            "new_string": "hello world"
        });

        let result = execute(input, &temp_path, false);
        assert!(result.is_ok(), "execute failed: {:?}", result.err());

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_replace_single_legacy() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "hello world").unwrap();
        let path = file.path().to_string_lossy().to_string();
        let work_dir = file.path().parent().unwrap().to_string_lossy().to_string();

        let input = serde_json::json!({
            "path": path.clone(),
            "old_string": "world",
            "new_string": "rust"
        });

        let result = execute(input, &work_dir, false);
        assert!(result.is_ok());
        assert!(fs::read_to_string(&path).unwrap().contains("hello rust"));
    }

    #[test]
    fn test_batch_edits() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "foo bar baz").unwrap();
        let path = file.path().to_string_lossy().to_string();
        let work_dir = file.path().parent().unwrap().to_string_lossy().to_string();

        let input = serde_json::json!({
            "path": path.clone(),
            "edits": [
                {"old_string": "foo", "new_string": "FOO"},
                {"old_string": "bar", "new_string": "BAR"}
            ]
        });

        let result = execute(input, &work_dir, false);
        assert!(result.is_ok(), "{:?}", result.err());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("FOO"));
        assert!(content.contains("BAR"));
        assert!(content.contains("baz"));
    }

    #[test]
    fn test_batch_fail_fast() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "foo bar").unwrap();
        let path = file.path().to_string_lossy().to_string();
        let work_dir = file.path().parent().unwrap().to_string_lossy().to_string();
        let original = fs::read_to_string(&path).unwrap();

        let input = serde_json::json!({
            "path": path.clone(),
            "edits": [
                {"old_string": "foo", "new_string": "FOO"},
                {"old_string": "MISSING", "new_string": "X"}
            ]
        });

        let result = execute(input, &work_dir, false);
        assert!(result.is_err());
        // disk must be untouched
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn test_replace_multiple_error() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "test test test").unwrap();
        let path = file.path().to_string_lossy().to_string();
        let work_dir = file.path().parent().unwrap().to_string_lossy().to_string();

        let input = serde_json::json!({
            "path": path.clone(),
            "old_string": "test",
            "new_string": "pass"
        });

        let result = execute(input, &work_dir, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("multiple matches"));
    }

    #[test]
    fn test_replace_all() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "test test test").unwrap();
        let path = file.path().to_string_lossy().to_string();
        let work_dir = file.path().parent().unwrap().to_string_lossy().to_string();

        let input = serde_json::json!({
            "path": path.clone(),
            "old_string": "test",
            "new_string": "pass",
            "replace_all": true
        });

        let result = execute(input, &work_dir, false);
        assert!(result.is_ok());
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("pass pass pass"));
    }

    #[test]
    fn test_edit_out_of_scope() {
        let input = serde_json::json!({
            "path": "/etc/passwd",
            "old_string": "root",
            "new_string": "hacked"
        });

        let result = execute(input, "/tmp", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of scope"));
    }
}
