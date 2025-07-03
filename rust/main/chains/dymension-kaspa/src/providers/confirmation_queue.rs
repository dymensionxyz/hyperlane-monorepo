use dym_kas_core::confirmation::ConfirmationFXG;
use hyperlane_core::H256;
use hyperlane_cosmos_rs::dymensionxyz::dymension::kas::WithdrawalId;
use std::sync::{Arc, Mutex};

pub struct ConfirmationQueue {
    queue: Vec<ConfirmationFXG>,
    mutex: Mutex<()>,
}

impl ConfirmationQueue {
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            mutex: Mutex::new(()),
        }
    }
    pub fn consume(&self) -> Vec<ConfirmationFXG> {
        let mut guard = self.mutex.lock().unwrap();
        std::mem::take(&mut *guard)
    }
    pub fn push(&self, fxg: ConfirmationFXG) {
        let mut guard = self.mutex.lock().unwrap();
        self.queue.push(fxg);
    }
}
