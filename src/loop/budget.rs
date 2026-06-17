pub use crate::types::TokenBudget;
use crate::get_convo_jsonl_path;

pub enum BudgetStatus {
    Ok { total: u32 },
    Warning { total: u32 },
    Exceeded { total: u32 },
}

impl BudgetStatus {
    pub fn total(&self) -> u32 {
        match self {
            BudgetStatus::Ok { total } => *total,
            BudgetStatus::Warning { total } => *total,
            BudgetStatus::Exceeded { total } => *total,
        }
    }
}

/// Estimate token usage by dividing conversation JSONL byte length by 3.
/// This is vendor-agnostic and conservative enough to be reliable at scale.
pub fn check(convo_id: &str, budget: &TokenBudget) -> BudgetStatus {
    let total = estimate_tokens(convo_id).unwrap_or(0);

    if total >= budget.hard_limit {
        BudgetStatus::Exceeded { total }
    } else if total >= budget.warn_threshold {
        BudgetStatus::Warning { total }
    } else {
        BudgetStatus::Ok { total }
    }
}

fn estimate_tokens(convo_id: &str) -> Option<u32> {
    let path = get_convo_jsonl_path(convo_id).ok()?;
    let bytes = std::fs::metadata(&path).ok()?.len();
    Some((bytes / 3) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::LogWriter;
    use crate::types::{ConvoEvent, MessageEvent, TokenBudget};
    use crate::TestEnv;
    use std::fs;

    fn setup_convo_with_chars(convo_id: &str, char_count: usize, base: &std::path::Path) {
        let convo_dir = base.join("conversations").join(convo_id);
        fs::create_dir_all(&convo_dir).unwrap();

        // Write enough message events to hit the target char count.
        let jsonl = convo_dir.join("conversation.jsonl");
        let chunk = "x".repeat(100);
        let mut written = 0usize;
        while written < char_count {
            let event = ConvoEvent::Message(MessageEvent::new("user", &chunk));
            LogWriter::append(&jsonl, &event).unwrap();
            written = fs::metadata(&jsonl).unwrap().len() as usize;
        }
    }

    #[test]
    fn test_ok() {
        let temp = tempfile::TempDir::new().unwrap();
        setup_convo_with_chars("c1", 30_000, temp.path());
        let _env = TestEnv::with_dir(temp);
        // 30_000 chars / 3 = 10_000 tokens — well under 80k warn threshold
        let budget = TokenBudget::default();
        assert!(matches!(check("c1", &budget), BudgetStatus::Ok { .. }));
    }

    #[test]
    fn test_warning() {
        let temp = tempfile::TempDir::new().unwrap();
        setup_convo_with_chars("c2", 270_000, temp.path());
        let _env = TestEnv::with_dir(temp);
        // 270_000 chars / 3 = 90_000 tokens — above 80k, below 120k
        let budget = TokenBudget::default();
        assert!(matches!(check("c2", &budget), BudgetStatus::Warning { .. }));
    }

    #[test]
    fn test_exceeded() {
        let temp = tempfile::TempDir::new().unwrap();
        setup_convo_with_chars("c3", 390_000, temp.path());
        let _env = TestEnv::with_dir(temp);
        // 390_000 chars / 3 = 130_000 tokens — above 120k hard limit
        let budget = TokenBudget::default();
        assert!(matches!(
            check("c3", &budget),
            BudgetStatus::Exceeded { .. }
        ));
    }
}
