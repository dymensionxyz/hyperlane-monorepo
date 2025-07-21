use dym_kas_core::confirmation::ConfirmationFXG;
use dym_kas_hardcode::tx::FINALITY_APPROX_WAIT_TIME;
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
    pub async fn consume(&self) -> Option<ConfirmationFXG> {
        if self.has_pending(){
            time::sleep(FINALITY_APPROX_WAIT_TIME).await;
        }
        let mut guard = self.mutex.lock().unwrap();
        std::mem::take(&mut *guard)
    }
    pub fn push(&self, fxg: ConfirmationFXG) {
        let mut guard = self.mutex.lock().unwrap();
        *guard = Some(fxg);
    }
    // New function to check if there's a pending FXG
    pub fn has_pending(&self) -> bool {
        let guard = self.mutex.lock().unwrap(); // Acquire lock
        guard.is_some() // Check if the Option contains a value
    }
}
