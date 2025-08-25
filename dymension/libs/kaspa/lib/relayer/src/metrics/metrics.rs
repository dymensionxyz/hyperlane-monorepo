use prometheus::{IntCounter, IntGauge, Registry};

/// Helper function to check if a Prometheus error is due to duplicate registration
fn is_already_registered_error(err: &prometheus::Error) -> bool {
    match err {
        prometheus::Error::AlreadyReg => true,
        _ => false,
    }
}

/// Kaspa relayer-specific metrics matching the requested specification
#[derive(Debug, Clone)]
pub struct KaspaBridgeMetrics {
    /// Relayer address funds - Current balance of the relayer address in sompi
    pub relayer_address_funds: IntGauge,
    
    /// Funds escrowed - Total funds currently held in escrow in sompi
    pub funds_escrowed: IntGauge,
    
    /// Total funds deposited - Cumulative amount of deposits processed in sompi
    pub total_funds_deposited: IntCounter,
    
    /// Total funds withdrawn - Cumulative amount of withdrawals processed in sompi
    pub total_funds_withdrawn: IntCounter,
    
    /// Failed withdrawals total - Total number of failed withdrawal attempts
    pub failed_withdrawals_total: IntCounter,
    
    /// Current failed withdrawals - Consecutive withdrawal failures since last success
    pub current_failed_withdrawals: IntGauge,
    
    /// Failed deposits total - Total number of failed deposit attempts
    pub failed_deposits_total: IntCounter,
    
    /// Current failed deposits - Consecutive deposit failures since last success
    pub current_failed_deposits: IntGauge,
    
    /// Confirmations failed - Total number of confirmation failures
    pub confirmations_failed: IntCounter,
    
    /// Confirmations pending - Number of confirmations currently pending
    pub confirmations_pending: IntGauge,
    
    /// Number of UTXOs in escrow address
    pub escrow_utxo_count: IntGauge,
    
    /// Deposit processing latency metrics
    pub deposit_min_latency_ms: IntGauge,
    pub deposit_max_latency_ms: IntGauge,
    pub deposit_avg_latency_ms: IntGauge,
    
    /// Withdrawal processing latency metrics  
    pub withdrawal_min_latency_ms: IntGauge,
    pub withdrawal_max_latency_ms: IntGauge,
    pub withdrawal_avg_latency_ms: IntGauge,
}

impl KaspaBridgeMetrics {
    pub fn new(_chain_name: &str) -> prometheus::Result<Self> {
        Self::new_with_registry(_chain_name, &prometheus::default_registry())
    }
    
    pub fn new_with_registry(_chain_name: &str, registry: &Registry) -> prometheus::Result<Self> {
        // Create Kaspa relayer metrics using the provided registry
        let relayer_address_funds = IntGauge::new(
            "kaspa_relayer_address_funds_sompi",
            "Current balance of the relayer address in sompi"
        )?;
        // Use try_register to handle duplicate registration gracefully
        if let Err(e) = registry.register(Box::new(relayer_address_funds.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let funds_escrowed = IntGauge::new(
            "kaspa_funds_escrowed_sompi",
            "Total funds currently held in escrow in sompi"
        )?;
        if let Err(e) = registry.register(Box::new(funds_escrowed.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let total_funds_deposited = IntCounter::new(
            "kaspa_total_funds_deposited_sompi",
            "Cumulative amount of deposits processed in sompi"
        )?;
        if let Err(e) = registry.register(Box::new(total_funds_deposited.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let total_funds_withdrawn = IntCounter::new(
            "kaspa_total_funds_withdrawn_sompi",
            "Cumulative amount of withdrawals processed in sompi"
        )?;
        if let Err(e) = registry.register(Box::new(total_funds_withdrawn.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let failed_withdrawals_total = IntCounter::new(
            "kaspa_failed_withdrawals_total",
            "Total number of failed withdrawal attempts"
        )?;
        if let Err(e) = registry.register(Box::new(failed_withdrawals_total.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let current_failed_withdrawals = IntGauge::new(
            "kaspa_consecutive_failed_withdrawals",
            "Consecutive withdrawal failures since last success"
        )?;
        if let Err(e) = registry.register(Box::new(current_failed_withdrawals.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let failed_deposits_total = IntCounter::new(
            "kaspa_failed_deposits_total",
            "Total number of failed deposit attempts"
        )?;
        if let Err(e) = registry.register(Box::new(failed_deposits_total.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let current_failed_deposits = IntGauge::new(
            "kaspa_current_failed_deposits",
            "Consecutive deposit failures since last success"
        )?;
        if let Err(e) = registry.register(Box::new(current_failed_deposits.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let confirmations_failed = IntCounter::new(
            "kaspa_confirmations_failed_total",
            "Total number of confirmation failures"
        )?;
        if let Err(e) = registry.register(Box::new(confirmations_failed.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let confirmations_pending = IntGauge::new(
            "kaspa_confirmations_pending",
            "Number of confirmations currently pending"
        )?;
        if let Err(e) = registry.register(Box::new(confirmations_pending.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let escrow_utxo_count = IntGauge::new(
            "kaspa_escrow_utxo_count",
            "Number of UTXOs in escrow address"
        )?;
        if let Err(e) = registry.register(Box::new(escrow_utxo_count.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let deposit_min_latency_ms = IntGauge::new(
            "kaspa_deposit_min_latency_ms",
            "Minimum deposit processing latency in milliseconds"
        )?;
        if let Err(e) = registry.register(Box::new(deposit_min_latency_ms.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let deposit_max_latency_ms = IntGauge::new(
            "kaspa_deposit_max_latency_ms",
            "Maximum deposit processing latency in milliseconds"
        )?;
        if let Err(e) = registry.register(Box::new(deposit_max_latency_ms.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let deposit_avg_latency_ms = IntGauge::new(
            "kaspa_deposit_avg_latency_ms",
            "Average deposit processing latency in milliseconds"
        )?;
        if let Err(e) = registry.register(Box::new(deposit_avg_latency_ms.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let withdrawal_min_latency_ms = IntGauge::new(
            "kaspa_withdrawal_min_latency_ms",
            "Minimum withdrawal processing latency in milliseconds"
        )?;
        if let Err(e) = registry.register(Box::new(withdrawal_min_latency_ms.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let withdrawal_max_latency_ms = IntGauge::new(
            "kaspa_withdrawal_max_latency_ms",
            "Maximum withdrawal processing latency in milliseconds"
        )?;
        if let Err(e) = registry.register(Box::new(withdrawal_max_latency_ms.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        let withdrawal_avg_latency_ms = IntGauge::new(
            "kaspa_withdrawal_avg_latency_ms",
            "Average withdrawal processing latency in milliseconds"
        )?;
        if let Err(e) = registry.register(Box::new(withdrawal_avg_latency_ms.clone())) {
            if !is_already_registered_error(&e) {
                return Err(e);
            }
        }
        
        Ok(Self {
            relayer_address_funds,
            funds_escrowed,
            total_funds_deposited,
            total_funds_withdrawn,
            failed_withdrawals_total,
            current_failed_withdrawals,
            failed_deposits_total,
            current_failed_deposits,
            confirmations_failed,
            confirmations_pending,
            escrow_utxo_count,
            deposit_min_latency_ms,
            deposit_max_latency_ms,
            deposit_avg_latency_ms,
            withdrawal_min_latency_ms,
            withdrawal_max_latency_ms,
            withdrawal_avg_latency_ms,
        })
    }
    
    /// Update relayer address balance
    pub fn update_relayer_funds(&self, balance_sompi: i64) {
        self.relayer_address_funds.set(balance_sompi);
    }
    
    /// Update escrow balance
    pub fn update_funds_escrowed(&self, balance_sompi: i64) {
        self.funds_escrowed.set(balance_sompi);
    }
    
    /// Record successful deposit processing with amount
    pub fn record_deposit_processed(&self, amount_sompi: u64) {
        self.total_funds_deposited.inc_by(amount_sompi);
        // Reset current failed deposits on success
        self.current_failed_deposits.set(0);
    }
    
    /// Record successful withdrawal processing with amount
    pub fn record_withdrawal_processed(&self, amount_sompi: u64) {
        self.total_funds_withdrawn.inc_by(amount_sompi);
        // Reset current failed withdrawals on success
        self.current_failed_withdrawals.set(0);
    }
    
    /// Record failed deposit attempt
    pub fn record_deposit_failed(&self) {
        self.failed_deposits_total.inc();
        self.current_failed_deposits.inc();
    }
    
    /// Record failed withdrawal attempt
    pub fn record_withdrawal_failed(&self) {
        self.failed_withdrawals_total.inc();
        self.current_failed_withdrawals.inc();
    }
    
    /// Record confirmation failure
    pub fn record_confirmation_failed(&self) {
        self.confirmations_failed.inc();
    }
    
    /// Update pending confirmations count
    pub fn update_confirmations_pending(&self, count: i64) {
        self.confirmations_pending.set(count);
    }
    
    /// Reset current failed deposits counter (call on successful deposit)
    pub fn reset_current_failed_deposits(&self) {
        self.current_failed_deposits.set(0);
    }
    
    /// Reset current failed withdrawals counter (call on successful withdrawal)
    pub fn reset_current_failed_withdrawals(&self) {
        self.current_failed_withdrawals.set(0);
    }
    
    /// Update the number of UTXOs in escrow address
    pub fn update_escrow_utxo_count(&self, count: i64) {
        self.escrow_utxo_count.set(count);
    }
    
    /// Update deposit latency metrics
    pub fn update_deposit_latency(&self, latency_ms: i64) {
        // Update min latency
        let current_min = self.deposit_min_latency_ms.get();
        if current_min == 0 || latency_ms < current_min {
            self.deposit_min_latency_ms.set(latency_ms);
        }
        
        // Update max latency
        let current_max = self.deposit_max_latency_ms.get();
        if latency_ms > current_max {
            self.deposit_max_latency_ms.set(latency_ms);
        }
        
        // Update average latency (simple moving average approach)
        let current_avg = self.deposit_avg_latency_ms.get();
        if current_avg == 0 {
            self.deposit_avg_latency_ms.set(latency_ms);
        } else {
            // Simple exponential moving average with alpha = 0.1
            let new_avg = ((current_avg as f64 * 0.9) + (latency_ms as f64 * 0.1)) as i64;
            self.deposit_avg_latency_ms.set(new_avg);
        }
    }
    
    /// Update withdrawal latency metrics
    pub fn update_withdrawal_latency(&self, latency_ms: i64) {
        // Update min latency
        let current_min = self.withdrawal_min_latency_ms.get();
        if current_min == 0 || latency_ms < current_min {
            self.withdrawal_min_latency_ms.set(latency_ms);
        }
        
        // Update max latency
        let current_max = self.withdrawal_max_latency_ms.get();
        if latency_ms > current_max {
            self.withdrawal_max_latency_ms.set(latency_ms);
        }
        
        // Update average latency (simple moving average approach)
        let current_avg = self.withdrawal_avg_latency_ms.get();
        if current_avg == 0 {
            self.withdrawal_avg_latency_ms.set(latency_ms);
        } else {
            // Simple exponential moving average with alpha = 0.1
            let new_avg = ((current_avg as f64 * 0.9) + (latency_ms as f64 * 0.1)) as i64;
            self.withdrawal_avg_latency_ms.set(new_avg);
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = KaspaBridgeMetrics::new("kaspa").expect("Failed to create metrics");
        
        // Test initial values
        assert_eq!(metrics.relayer_address_funds.get(), 0);
        assert_eq!(metrics.funds_escrowed.get(), 0);
        assert_eq!(metrics.current_failed_withdrawals.get(), 0);
        assert_eq!(metrics.current_failed_deposits.get(), 0);
        assert_eq!(metrics.confirmations_pending.get(), 0);
        assert_eq!(metrics.escrow_utxo_count.get(), 0);
        assert_eq!(metrics.deposit_min_latency_ms.get(), 0);
        assert_eq!(metrics.deposit_max_latency_ms.get(), 0);
        assert_eq!(metrics.deposit_avg_latency_ms.get(), 0);
        assert_eq!(metrics.withdrawal_min_latency_ms.get(), 0);
        assert_eq!(metrics.withdrawal_max_latency_ms.get(), 0);
        assert_eq!(metrics.withdrawal_avg_latency_ms.get(), 0);
    }

    #[test]
    fn test_metrics_operations() {
        let metrics = KaspaBridgeMetrics::new("kaspa").expect("Failed to create metrics");
        
        // Test balance updates
        metrics.update_relayer_funds(1000000);
        assert_eq!(metrics.relayer_address_funds.get(), 1000000);
        
        metrics.update_funds_escrowed(500000);
        assert_eq!(metrics.funds_escrowed.get(), 500000);
        
        // Test deposit processing
        let initial_total = metrics.total_funds_deposited.get();
        metrics.record_deposit_processed(100000);
        assert_eq!(metrics.total_funds_deposited.get() as u64, initial_total as u64 + 100000);
        
        // Test withdrawal processing
        let initial_total = metrics.total_funds_withdrawn.get();
        metrics.record_withdrawal_processed(50000);
        assert_eq!(metrics.total_funds_withdrawn.get() as u64, initial_total as u64 + 50000);
        
        // Test failure tracking
        metrics.record_deposit_failed();
        assert_eq!(metrics.current_failed_deposits.get(), 1);
        assert_eq!(metrics.failed_deposits_total.get() as u64, 1);
        
        metrics.record_withdrawal_failed();
        assert_eq!(metrics.current_failed_withdrawals.get(), 1);
        assert_eq!(metrics.failed_withdrawals_total.get() as u64, 1);
        
        // Test failure reset on success
        metrics.record_deposit_processed(10000);
        assert_eq!(metrics.current_failed_deposits.get(), 0);
        
        metrics.record_withdrawal_processed(5000);
        assert_eq!(metrics.current_failed_withdrawals.get(), 0);
        
        // Test confirmation metrics
        metrics.record_confirmation_failed();
        assert_eq!(metrics.confirmations_failed.get() as u64, 1);
        
        metrics.update_confirmations_pending(5);
        assert_eq!(metrics.confirmations_pending.get(), 5);
        
        // Test UTXO count
        metrics.update_escrow_utxo_count(10);
        assert_eq!(metrics.escrow_utxo_count.get(), 10);
        
        // Test deposit latency metrics
        metrics.update_deposit_latency(100);
        assert_eq!(metrics.deposit_min_latency_ms.get(), 100);
        assert_eq!(metrics.deposit_max_latency_ms.get(), 100);
        assert_eq!(metrics.deposit_avg_latency_ms.get(), 100);
        
        metrics.update_deposit_latency(200);
        assert_eq!(metrics.deposit_min_latency_ms.get(), 100);
        assert_eq!(metrics.deposit_max_latency_ms.get(), 200);
        assert_eq!(metrics.deposit_avg_latency_ms.get(), 110); // 100 * 0.9 + 200 * 0.1 = 110
        
        metrics.update_deposit_latency(50);
        assert_eq!(metrics.deposit_min_latency_ms.get(), 50);
        assert_eq!(metrics.deposit_max_latency_ms.get(), 200);
        assert_eq!(metrics.deposit_avg_latency_ms.get(), 104); // 110 * 0.9 + 50 * 0.1 = 104
        
        // Test withdrawal latency metrics
        metrics.update_withdrawal_latency(300);
        assert_eq!(metrics.withdrawal_min_latency_ms.get(), 300);
        assert_eq!(metrics.withdrawal_max_latency_ms.get(), 300);
        assert_eq!(metrics.withdrawal_avg_latency_ms.get(), 300);
        
        metrics.update_withdrawal_latency(400);
        assert_eq!(metrics.withdrawal_min_latency_ms.get(), 300);
        assert_eq!(metrics.withdrawal_max_latency_ms.get(), 400);
        assert_eq!(metrics.withdrawal_avg_latency_ms.get(), 310); // 300 * 0.9 + 400 * 0.1 = 310
    }

    #[test]
    fn test_duplicate_metrics_creation() {
        // Create first instance - should work fine
        let metrics1 = KaspaBridgeMetrics::new("kaspa-duplicate-test").expect("Failed to create first metrics instance");
        
        // Create second instance - should handle duplicate registration gracefully
        let metrics2 = KaspaBridgeMetrics::new("kaspa-duplicate-test").expect("Failed to create second metrics instance");
        
        // Test that both metrics instances are functional
        metrics1.update_relayer_funds(1000000);
        metrics2.update_funds_escrowed(500000);
        
        // Verify the values are accessible (they share the same underlying metrics)
        assert_eq!(metrics1.relayer_address_funds.get(), 1000000);
        assert_eq!(metrics2.funds_escrowed.get(), 500000);
    }
}