use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonError {
    pub error: String,
    pub message: String,
}

impl JsonError {
    pub fn new(error_code: &str, message: &str) -> Self {
        JsonError {
            error: error_code.to_string(),
            message: message.to_string(),
        }
    }

    pub fn to_stderr(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            eprintln!("{}", json);
        } else {
            eprintln!("{{\"error\": \"serialization_failed\", \"message\": \"Failed to serialize error\"}}");
        }
    }

    pub fn config_not_found() -> Self {
        JsonError::new(
            "config_not_found",
            "No profile configured. Run: orchid config use <profile>",
        )
    }

    pub fn invalid_config(reason: &str) -> Self {
        JsonError::new("invalid_config", reason)
    }

    pub fn file_error(reason: &str) -> Self {
        JsonError::new("file_error", reason)
    }

    pub fn internal_error(reason: &str) -> Self {
        JsonError::new("internal_error", reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize() {
        let err = JsonError::new("test_error", "test message");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("test_error"));
        assert!(json.contains("test message"));
    }

    #[test]
    fn test_config_not_found() {
        let err = JsonError::config_not_found();
        assert_eq!(err.error, "config_not_found");
    }
}
