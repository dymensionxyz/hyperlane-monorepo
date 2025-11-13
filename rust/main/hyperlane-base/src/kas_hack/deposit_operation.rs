use dymension_kaspa::{conf::KaspaTimeConfig, Deposit};
use std::cmp::Ordering;
use std::time::{Duration, Instant};
use tracing::error;

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
        self.next_attempt_after
            .cmp(&other.next_attempt_after)
            .reverse()
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

    pub fn mark_failed(&mut self, cfg: &KaspaTimeConfig, custom_delay: Option<Duration>) {
        self.retry_count += 1;

        let delay = match custom_delay {
            Some(d) => d,
            None => {
                let delay_secs = if self.retry_count == 1 {
                    cfg.base_retry_delay_secs
                } else {
                    let exponential_delay = (cfg.base_retry_delay_secs as f64)
                        * cfg.retry_delay_exponent.powi((self.retry_count - 1) as i32);
                    exponential_delay.min(cfg.max_retry_delay_secs as f64) as u64
                };
                Duration::from_secs(delay_secs)
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

    pub fn reset_attempts(&mut self) {
        self.retry_count = 0;
        self.next_attempt_after = Instant::now();
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
        self.operations.push(op);
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
        self.operations.push(op);
    }

    pub fn len(&self) -> usize {
        self.operations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}
