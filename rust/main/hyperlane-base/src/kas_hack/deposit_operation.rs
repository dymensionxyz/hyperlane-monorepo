use dymension_kaspa::{conf::KaspaTimeConfig, Deposit};
use std::cmp::Ordering;
use std::time::{Duration, Instant};
use tracing::{debug, error};

#[derive(Debug, Clone)]
pub struct DepositOperation {
    pub deposit: Deposit,
    pub escrow_address: String,
    pub retry_count: u32,
    pub next_attempt_after: Option<Instant>,
    pub created_at: Instant,
}

impl PartialEq for DepositOperation {
    fn eq(&self, other: &Self) -> bool {
        self.next_attempt_after == other.next_attempt_after
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
        match (self.next_attempt_after, other.next_attempt_after) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(a), Some(b)) => b.cmp(&a),
        }
    }
}

impl DepositOperation {
    pub fn new(deposit: Deposit, escrow_address: String) -> Self {
        Self {
            deposit,
            escrow_address,
            retry_count: 0,
            next_attempt_after: None,
            created_at: Instant::now(),
        }
    }

    pub fn is_ready(&self) -> bool {
        match self.next_attempt_after {
            Some(next_attempt) => Instant::now() >= next_attempt,
            None => true,
        }
    }

    pub fn mark_failed(&mut self, cfg: &KaspaTimeConfig) {
        self.retry_count += 1;
        let delay_secs = if self.retry_count == 1 {
            cfg.base_retry_delay_secs
        } else {
            let exponential_delay = (cfg.base_retry_delay_secs as f64)
                * cfg.retry_delay_exponent.powi((self.retry_count - 1) as i32);
            exponential_delay.min(cfg.max_retry_delay_secs as f64) as u64
        };
        self.next_attempt_after = Some(Instant::now() + Duration::from_secs(delay_secs));
        error!(
            deposit_id = %self.deposit.id,
            retry_count = self.retry_count,
            retry_after_secs = delay_secs,
            "Deposit operation failed, scheduling retry"
        );
    }

    pub fn mark_failed_with_custom_delay(&mut self, delay: Duration, reason: &str) {
        self.retry_count += 1;
        self.next_attempt_after = Some(Instant::now() + delay);
        error!(
            deposit_id = %self.deposit.id,
            retry_count = self.retry_count,
            retry_after_secs = delay.as_secs_f64(),
            reason = %reason,
            "Deposit operation failed with custom delay"
        );
    }

    pub fn reset_attempts(&mut self) {
        self.retry_count = 0;
        self.next_attempt_after = None;
    }
}

#[derive(Debug)]
pub struct DepositOpQueue {
    operations: std::collections::BinaryHeap<DepositOperation>,
}

impl DepositOpQueue {
    pub fn new() -> Self {
        Self {
            operations: std::collections::BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, op: DepositOperation) {
        let id = op.deposit.id;
        self.operations.push(op);
        debug!("Added deposit operation to queue: {}", id);
    }

    pub fn pop_ready(&mut self) -> Option<DepositOperation> {
        if let Some(op) = self.operations.peek() {
            if op.is_ready() {
                return self.operations.pop();
            }
        }
        None
    }

    pub fn requeue(&mut self, op: DepositOperation) {
        let id = op.deposit.id;
        self.operations.push(op);
        debug!("Re-queued deposit operation: {}", id);
    }

    pub fn len(&self) -> usize {
        self.operations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}
