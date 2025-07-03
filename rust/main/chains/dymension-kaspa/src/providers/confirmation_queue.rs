use dym_kas_core::confirmation::ConfirmationFXG;
use std::sync::Mutex;

pub struct ConfirmationQueue {
    mutex: Mutex<Vec<ConfirmationFXG>>,
}

impl ConfirmationQueue {
    pub fn new() -> Self {
        Self {
            mutex: Mutex::new(Vec::new()),
        }
    }
    pub fn consume(&mut self) -> Vec<ConfirmationFXG> {
        let mut guard = self.mutex.lock().unwrap();
        std::mem::take(&mut *guard)
    }
    pub fn push(&mut self, fxg: ConfirmationFXG) {
        let mut guard = self.mutex.lock().unwrap();
        guard.push(fxg);
    }
}
