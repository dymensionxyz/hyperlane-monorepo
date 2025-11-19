use std::time::Duration;
use url::Url;

/// Configuration for Kaspa bridge connections
#[derive(Debug, Clone)]
pub struct BridgeConnectionConfig {
    pub wallet_secret: String,
    pub wallet_dir: Option<String>,
    pub kaspa_urls_wrpc: Vec<String>,
    pub kaspa_urls_rest: Vec<Url>,
    pub validator_pub_keys: Vec<String>,
    pub multisig_threshold_kaspa: usize,
    pub min_deposit_sompi: u128,
}

/// Configuration for deposit polling and retry
#[derive(Debug, Clone)]
pub struct DepositPollConfig {
    pub poll_interval: Duration,
    pub deposit_look_back: Option<Duration>,
}

impl Default for DepositPollConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            deposit_look_back: None,
        }
    }
}

impl DepositPollConfig {
    pub fn lower_bound_unix_time(&self) -> Option<i64> {
        self.deposit_look_back.map(|dur| {
            let now = kaspa_core::time::unix_now() as i64;
            now - dur.as_millis() as i64
        })
    }
}

/// Configuration for withdrawal transaction building
#[derive(Debug, Clone)]
pub struct WithdrawalConfig {
    pub tx_fee_multiplier: f64,
}

impl Default for WithdrawalConfig {
    fn default() -> Self {
        Self {
            tx_fee_multiplier: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deposit_poll_config_default() {
        let cfg = DepositPollConfig::default();
        assert_eq!(cfg.poll_interval, Duration::from_secs(5));
        assert!(cfg.deposit_look_back.is_none());
    }

    #[test]
    fn test_deposit_poll_config_lower_bound() {
        let cfg = DepositPollConfig {
            poll_interval: Duration::from_secs(5),
            deposit_look_back: Some(Duration::from_secs(3600)),
        };

        let lower_bound = cfg.lower_bound_unix_time();
        assert!(lower_bound.is_some());
    }

    #[test]
    fn test_withdrawal_config_default() {
        let cfg = WithdrawalConfig::default();
        assert_eq!(cfg.tx_fee_multiplier, 1.0);
    }
}
