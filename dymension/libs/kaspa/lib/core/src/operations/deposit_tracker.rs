use crate::api::client::Deposit;
use kaspa_consensus_core::tx::TransactionId;
use rand::Rng;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::time::{Duration, Instant};
use tracing::error;

/// Configuration for deposit retry timing
#[derive(Debug, Clone)]
pub struct DepositRetryConfig {
    pub retry_delay_base: Duration,
    pub retry_delay_exponent: f64,
    pub retry_delay_max: Duration,
}

impl Default for DepositRetryConfig {
    fn default() -> Self {
        Self {
            retry_delay_base: Duration::from_secs(30),
            retry_delay_exponent: 2.0,
            retry_delay_max: Duration::from_secs(3600),
        }
    }
}

impl DepositRetryConfig {
    pub fn new(
        retry_delay_base: Duration,
        retry_delay_exponent: f64,
        retry_delay_max: Duration,
    ) -> Self {
        Self {
            retry_delay_base,
            retry_delay_exponent,
            retry_delay_max,
        }
    }
}

/// A deposit operation being tracked for submission
#[derive(Debug, Clone)]
pub struct DepositOperation {
    pub deposit: Deposit,
    pub escrow_address: String,
    pub retry_count: u32,
    pub next_attempt_after: Instant,
    pub created_at: Instant,
}

impl PartialEq for DepositOperation {
    fn eq(&self, other: &Self) -> bool {
        self.deposit.id == other.deposit.id
    }
}

impl Eq for DepositOperation {}

impl PartialOrd for DepositOperation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DepositOperation {
    fn cmp(&self, other: &Self) -> Ordering {
        match self
            .next_attempt_after
            .cmp(&other.next_attempt_after)
            .reverse()
        {
            Ordering::Equal => self.deposit.id.cmp(&other.deposit.id),
            other => other,
        }
    }
}

impl DepositOperation {
    pub fn new(deposit: Deposit, escrow_address: String) -> Self {
        let now = Instant::now();
        Self {
            deposit,
            escrow_address,
            retry_count: 0,
            next_attempt_after: now,
            created_at: now,
        }
    }

    pub fn is_ready(&self) -> bool {
        Instant::now() >= self.next_attempt_after
    }

    pub fn mark_failed(&mut self, cfg: &DepositRetryConfig, custom_delay: Option<Duration>) {
        self.retry_count += 1;

        let delay = match custom_delay {
            Some(d) => d,
            None => {
                let base_delay = if self.retry_count == 1 {
                    cfg.retry_delay_base
                } else {
                    let base_secs = cfg.retry_delay_base.as_secs_f64();
                    let exponential_delay =
                        base_secs * cfg.retry_delay_exponent.powi((self.retry_count - 1) as i32);
                    let max_secs = cfg.retry_delay_max.as_secs_f64();
                    Duration::from_secs_f64(exponential_delay.min(max_secs))
                };

                let mut rng = rand::thread_rng();
                let jitter = rng.gen_range(0.75..=1.25);
                let delay_secs = base_delay.as_secs_f64() * jitter;
                Duration::from_secs_f64(delay_secs)
            }
        };

        self.next_attempt_after = Instant::now() + delay;
        error!(
            deposit_id = %self.deposit.id,
            retry_count = self.retry_count,
            retry_after_secs = delay.as_secs_f64(),
            "Deposit operation failed"
        );
    }
}

/// Tracks deposit operations with retry logic
#[derive(Debug)]
pub struct DepositTracker {
    seen: HashSet<TransactionId>,
    pending: BinaryHeap<DepositOperation>,
}

impl DepositTracker {
    pub fn new() -> Self {
        Self {
            seen: HashSet::new(),
            pending: BinaryHeap::new(),
        }
    }

    pub fn has_seen(&self, deposit: &Deposit) -> bool {
        self.seen.contains(&deposit.id)
    }

    pub fn track(&mut self, deposit: Deposit, escrow_address: String) -> bool {
        if self.seen.insert(deposit.id) {
            let op = DepositOperation::new(deposit, escrow_address);
            self.pending.push(op);
            true
        } else {
            false
        }
    }

    pub fn pop_ready(&mut self) -> Option<DepositOperation> {
        if let Some(op) = self.pending.peek() {
            if op.is_ready() {
                return self.pending.pop();
            }
        }
        None
    }

    pub fn requeue(&mut self, op: DepositOperation) {
        self.pending.push(op);
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn seen_count(&self) -> usize {
        self.seen.len()
    }
}

impl Default for DepositTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaspa_hashes::Hash as KaspaHash;

    fn create_test_deposit(id_byte: u8) -> Deposit {
        Deposit {
            payload: Some("test".to_string()),
            id: TransactionId::from(KaspaHash::from_bytes([id_byte; 32])),
            time: 0,
            accepted: true,
            outputs: vec![],
            accepting_block_hash: "test".to_string(),
            accepting_block_time: 0,
            accepting_block_blue_score: 0,
            block_hashes: vec![],
        }
    }

    #[test]
    fn test_deposit_tracker_tracks_new_deposits() {
        let mut tracker = DepositTracker::new();
        let deposit = create_test_deposit(1);

        assert!(!tracker.has_seen(&deposit));
        assert!(tracker.track(deposit.clone(), "escrow".to_string()));
        assert!(tracker.has_seen(&deposit));
        assert_eq!(tracker.pending_count(), 1);
    }

    #[test]
    fn test_deposit_tracker_ignores_duplicates() {
        let mut tracker = DepositTracker::new();
        let deposit = create_test_deposit(1);

        assert!(tracker.track(deposit.clone(), "escrow".to_string()));
        assert!(!tracker.track(deposit.clone(), "escrow".to_string()));
        assert_eq!(tracker.pending_count(), 1);
    }

    #[test]
    fn test_deposit_operation_retry_logic() {
        let deposit = create_test_deposit(1);
        let mut op = DepositOperation::new(deposit, "escrow".to_string());
        let cfg = DepositRetryConfig::default();

        assert_eq!(op.retry_count, 0);
        op.mark_failed(&cfg, None);
        assert_eq!(op.retry_count, 1);

        let first_delay = op.next_attempt_after - op.created_at;
        assert!(first_delay >= cfg.retry_delay_base);
    }
}
