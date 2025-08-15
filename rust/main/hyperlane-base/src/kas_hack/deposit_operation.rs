use dymension_kaspa::Deposit;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Configuration for deposit queue behavior
#[derive(Debug, Clone)]
pub struct DepositQueueConfig {
    pub max_retries: u32,
    pub max_queue_size: usize,
    pub base_retry_delay_secs: u64,
    pub secs_per_confirmation: f64,
}

impl DepositQueueConfig {
    pub fn from_env() -> Self {
        Self {
            max_retries: std::env::var("KASPA_MAX_RETRIES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50),
            max_queue_size: std::env::var("KASPA_MAX_QUEUE_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10000),
            base_retry_delay_secs: std::env::var("KASPA_BASE_RETRY_DELAY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            secs_per_confirmation: std::env::var("KASPA_SECS_PER_CONFIRMATION")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.1),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositOperation {
    pub deposit: Deposit,
    pub escrow_address: String,
    pub retry_count: u32,
    #[serde(skip)]
    pub next_attempt_after: Option<Instant>,
    /// Timestamp in seconds since epoch for persistence
    pub next_attempt_timestamp: Option<u64>,
}

impl DepositOperation {
    pub fn new(deposit: Deposit, escrow_address: String) -> Self {
        Self {
            deposit,
            escrow_address,
            retry_count: 0,
            next_attempt_after: None,
            next_attempt_timestamp: None,
        }
    }

    pub fn is_ready(&self) -> bool {
        match self.next_attempt_after {
            Some(next_attempt) => Instant::now() >= next_attempt,
            None => true,
        }
    }

    pub fn is_expired(&self, max_retries: u32) -> bool {
        self.retry_count >= max_retries
    }

    pub fn mark_failed(&mut self, base_delay_secs: u64) {
        self.retry_count += 1;
        // Exponential backoff: base, 2*base, 4*base, 8*base (capped)
        let delay_secs = base_delay_secs * (1 << (self.retry_count - 1).min(3));
        self.set_next_attempt(Duration::from_secs(delay_secs));
        info!(
            "Deposit operation failed, will retry in {}s (attempt {}): {}",
            delay_secs, self.retry_count, self.deposit.id
        );
    }

    /// Mark failed with custom retry timing (for finality-based delays)
    pub fn mark_failed_with_custom_delay(&mut self, delay: Duration, reason: &str) {
        self.retry_count += 1;
        self.set_next_attempt(delay);
        info!(
            "Deposit operation failed ({}), will retry in {:.1}s (attempt {}): {}",
            reason,
            delay.as_secs_f64(),
            self.retry_count,
            self.deposit.id
        );
    }

    fn set_next_attempt(&mut self, delay: Duration) {
        self.next_attempt_after = Some(Instant::now() + delay);
        self.next_attempt_timestamp = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + delay.as_secs(),
        );
    }

    pub fn reset_attempts(&mut self) {
        self.retry_count = 0;
        self.next_attempt_after = None;
        self.next_attempt_timestamp = None;
    }

    /// Restore from persisted state
    pub fn restore_timing(&mut self) {
        if let Some(timestamp) = self.next_attempt_timestamp {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if timestamp > now {
                self.next_attempt_after =
                    Some(Instant::now() + Duration::from_secs(timestamp - now));
            } else {
                self.next_attempt_after = None;
            }
        }
    }
}

/// Simple operation queue for managing deposit retries
#[derive(Debug, Serialize, Deserialize)]
pub struct DepositOpQueue {
    operations: std::collections::VecDeque<DepositOperation>,
    #[serde(skip)]
    config: DepositQueueConfig,
}

impl DepositOpQueue {
    pub fn new() -> Self {
        Self {
            operations: std::collections::VecDeque::new(),
            config: DepositQueueConfig::from_env(),
        }
    }

    pub fn with_config(config: DepositQueueConfig) -> Self {
        Self {
            operations: std::collections::VecDeque::new(),
            config,
        }
    }

    pub fn push(&mut self, operation: DepositOperation) -> Result<(), &'static str> {
        if self.operations.len() >= self.config.max_queue_size {
            warn!(
                "Queue full: {} operations (max: {})",
                self.operations.len(),
                self.config.max_queue_size
            );
            return Err("Queue full");
        }

        if operation.is_expired(self.config.max_retries) {
            warn!(
                "Deposit {} exceeded max retries ({})",
                operation.deposit.id, self.config.max_retries
            );
            return Err("Max retries exceeded");
        }

        let operation_id = operation.deposit.id;
        self.operations.push_back(operation);
        debug!("Added deposit operation to queue: {}", operation_id);
        Ok(())
    }

    pub fn pop_ready(&mut self) -> Option<DepositOperation> {
        if let Some(pos) = self.operations.iter().position(|op| op.is_ready()) {
            self.operations.remove(pos)
        } else {
            None
        }
    }

    pub fn requeue(&mut self, operation: DepositOperation) -> Result<(), &'static str> {
        if operation.is_expired(self.config.max_retries) {
            warn!(
                "Cannot requeue: deposit {} exceeded max retries ({})",
                operation.deposit.id, self.config.max_retries
            );
            return Err("Max retries exceeded");
        }

        let operation_id = operation.deposit.id;
        self.operations.push_back(operation);
        debug!("Re-queued deposit operation: {}", operation_id);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.operations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    pub fn config(&self) -> &DepositQueueConfig {
        &self.config
    }

    /// Restore timing for all operations after loading from DB
    pub fn restore_all_timing(&mut self) {
        for op in &mut self.operations {
            op.restore_timing();
        }
    }
}
