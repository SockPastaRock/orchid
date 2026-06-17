use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use std::io::Write;

/// Manages `stream.state` inside a conversation directory.
///
/// Format: `<unix_timestamp_secs> <chunk_count>\n`
/// - Created when streaming begins, deleted on completion or drop.
/// - External tools can poll mtime or the counter for liveness.
pub struct StreamState {
    path: PathBuf,
    chunk_count: u64,
}

impl StreamState {
    pub fn create(convo_dir: &PathBuf) -> Self {
        let prior = Self::read_chunk_count(&convo_dir);
        let path = convo_dir.join("stream.state");
        let mut state = StreamState { path, chunk_count: prior };
        state.tick();
        state
    }

    fn read_chunk_count(convo_dir: &PathBuf) -> u64 {
        let path = convo_dir.join("stream.state");
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.split_whitespace().nth(1).and_then(|n| n.parse().ok()))
            .unwrap_or(0)
    }

    pub fn tick(&mut self) {
        self.chunk_count += 1;
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Ok(mut f) = fs::File::create(&self.path) {
            let _ = write!(f, "{} {}\n", ts, self.chunk_count);
        }
    }
}
