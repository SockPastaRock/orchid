use crate::types::ConvoEvent;
use chrono::Utc;
use serde_json::json;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

// ── Conversation JSONL ────────────────────────────────────────────────────────

pub struct LogWriter;

impl LogWriter {
    pub fn append<P: AsRef<Path>>(path: P, event: &ConvoEvent) -> Result<String, String> {
        let entry = serde_json::to_string(event)
            .map_err(|e| format!("failed to serialize event: {}", e))?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("failed to open log file: {}", e))?;

        writeln!(file, "{}", entry).map_err(|e| format!("failed to write log entry: {}", e))?;

        let event_id = match event {
            ConvoEvent::Message(e) => e.event_id.clone(),
            ConvoEvent::ToolCall(e) => e.event_id.clone(),
            ConvoEvent::ToolResult(e) => e.event_id.clone(),
            ConvoEvent::Reasoning(e) => e.event_id.clone(),
        };

        Ok(event_id)
    }
}

pub struct LogReader;

impl LogReader {
    pub fn read_lines<P: AsRef<Path>>(path: P) -> Result<Vec<ConvoEvent>, String> {
        let file =
            std::fs::File::open(path).map_err(|e| format!("failed to open log file: {}", e))?;

        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| format!("failed to read line: {}", e))?;

            if line.trim().is_empty() {
                continue;
            }

            let event: ConvoEvent =
                serde_json::from_str(&line).map_err(|e| format!("failed to parse JSON: {}", e))?;

            events.push(event);
        }

        Ok(events)
    }
}

// ── Diagnostic logger ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn from_config_str(s: Option<&str>) -> Self {
        match s {
            Some("debug") => LogLevel::Debug,
            _ => LogLevel::Info,
        }
    }
}

/// Runtime diagnostic logger. Writes newline-delimited JSON to `orchid.log`
/// inside the conversation directory. Silently no-ops if the path is unset.
pub struct DiagLogger {
    path: Option<PathBuf>,
    level: LogLevel,
}

impl DiagLogger {
    /// Logger that writes to `<convo_dir>/orchid.log`.
    pub fn for_convo(convo_dir: PathBuf, level: LogLevel) -> Self {
        DiagLogger {
            path: Some(convo_dir.join("orchid.log")),
            level,
        }
    }

    /// No-op logger (e.g. outside a conversation context).
    pub fn noop() -> Self {
        DiagLogger {
            path: None,
            level: LogLevel::Info,
        }
    }

    pub fn debug(&self, event: &str, detail: &str) {
        if self.level <= LogLevel::Debug {
            self.write("DEBUG", event, detail);
        }
    }

    pub fn info(&self, event: &str, detail: &str) {
        if self.level <= LogLevel::Info {
            self.write("INFO", event, detail);
        }
    }

    pub fn warn(&self, event: &str, detail: &str) {
        if self.level <= LogLevel::Warn {
            self.write("WARN", event, detail);
        }
    }

    pub fn error(&self, event: &str, detail: &str) {
        self.write("ERROR", event, detail);
    }

    fn write(&self, level: &str, event: &str, detail: &str) {
        let Some(ref path) = self.path else { return };

        let entry = json!({
            "ts": Utc::now(),
            "level": level,
            "event": event,
            "detail": detail,
        });

        // Best-effort — diagnostic failures must not crash the main loop.
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{}", entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MessageEvent;
    use tempfile::TempDir;

    #[test]
    fn test_append_and_read() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = TempDir::new()?;
        let log_path = tmp_dir.path().join("test.jsonl");

        let e1 = ConvoEvent::Message(MessageEvent::new("user", "hello"));
        let e2 = ConvoEvent::Message(MessageEvent::new("assistant", "hi there"));

        LogWriter::append(&log_path, &e1)?;
        LogWriter::append(&log_path, &e2)?;

        let events = LogReader::read_lines(&log_path)?;

        assert_eq!(events.len(), 2);
        match &events[0] {
            ConvoEvent::Message(e) => {
                assert_eq!(e.message.role, "user");
                assert_eq!(e.message.content, "hello");
            }
            _ => panic!("expected Message event"),
        }
        match &events[1] {
            ConvoEvent::Message(e) => {
                assert_eq!(e.message.role, "assistant");
                assert_eq!(e.message.content, "hi there");
            }
            _ => panic!("expected Message event"),
        }

        Ok(())
    }
}
