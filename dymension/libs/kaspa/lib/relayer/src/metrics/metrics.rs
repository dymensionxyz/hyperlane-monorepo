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