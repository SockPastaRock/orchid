use crate::r#loop::lifecycle;

/// RAII guard that ensures `on_run_end` is called when a run completes or fails.
///
/// Call `disarm()` after a successful run to prevent the cleanup from firing.
pub struct RunGuard<'a> {
    convo_id: &'a str,
    disarmed: bool,
}

impl<'a> RunGuard<'a> {
    pub fn new(convo_id: &'a str) -> Self {
        Self {
            convo_id,
            disarmed: false,
        }
    }

    pub fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        if !self.disarmed {
            let _ = lifecycle::on_run_end(self.convo_id);
        }
    }
}
