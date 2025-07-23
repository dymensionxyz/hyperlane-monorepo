use dym_kas_core::confirmation::ConfirmationFXG;
use std::sync::Mutex;
use tokio::time;

#[derive(Debug)]
pub struct PendingConfirmation {
    mutex: Mutex<Option<ConfirmationFXG>>,
}

impl PendingConfirmation {
    pub fn new() -> Self {
        Self {
            mutex: Mutex::new(None),
        }
    }

    /// consume waits a FINALITY_APPROX_WAIT_TIME before returning ConfirmationFXG when there is a pending one
    pub async fn consume(&self) -> Option<ConfirmationFXG> {
        let mut guard = self.mutex.lock().unwrap();
        std::mem::take(&mut *guard)
    }
    pub fn push(&self, fxg: ConfirmationFXG) {
        let mut guard = self.mutex.lock().unwrap();
        *guard = Some(fxg);
    }
    /// has_pending checks if there's a pending ConfirmationFXG
    pub fn has_pending(&self) -> bool {
        let guard = self.mutex.lock().unwrap(); // Acquire lock
        guard.is_some() // Check if the Option contains a value
    }
}
