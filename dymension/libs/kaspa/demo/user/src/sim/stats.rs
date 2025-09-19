use crate::sim::util::som_to_kas;
use eyre::Error;
use kaspa_addresses::Address;
use kaspa_consensus_core::tx::TransactionId;
use serde::{Serialize, Serializer};
use std::fs::File;
use std::time::Duration;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tendermint::hash::Hash as TendermintHash;
use tracing::info;

// Custom serializer for SystemTime to milliseconds since epoch
fn serialize_systemtime_as_millis<S>(
    time: &Option<SystemTime>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match time {
        Some(t) => {
            let millis = t
                .duration_since(UNIX_EPOCH)
                .map_err(serde::ser::Error::custom)?
                .as_millis() as u64;
            serializer.serialize_u64(millis)
        }
        None => serializer.serialize_none(),
    }
}

pub fn render_stats(stats: Vec<RoundTripStats>, total_spend: u64, total_ops: u64) {
    info!("Total spend: {}", som_to_kas(total_spend));
    info!("Total ops: {}", total_ops);
    for s in stats {
        info!("=== Round Trip Stats ===");
        info!("op_id: {}, value: {}", s.op_id, som_to_kas(s.value));
        info!("stage: {:?}", s.stage());

        // Show timestamps in milliseconds
        if let Some(time) = s.kaspa_deposit_tx_time {
            let millis = time.duration_since(UNIX_EPOCH).unwrap().as_millis();
            info!("kaspa_deposit_tx_time: {}ms", millis);
        }
        if let Some(time) = s.deposit_credit_time {
            let millis = time.duration_since(UNIX_EPOCH).unwrap().as_millis();
            info!("deposit_credit_time: {}ms", millis);
        }
        if let Some(time) = s.hub_withdraw_tx_time {
            let millis = time.duration_since(UNIX_EPOCH).unwrap().as_millis();
            info!("hub_withdraw_tx_time: {}ms", millis);
        }
        if let Some(time) = s.withdraw_credit_time {
            let millis = time.duration_since(UNIX_EPOCH).unwrap().as_millis();
            info!("withdraw_credit_time: {}ms", millis);
        }

        // Show durations
        if s.deposit_credit_time.is_some() {
            info!("deposit duration: {}ms", s.deposit_time().as_millis());
        }
        if s.withdraw_credit_time.is_some() {
            info!("withdraw duration: {}ms", s.withdraw_time().as_millis());
        }

        // Show addresses
        if let Some(ref addr) = s.deposit_addr_hub {
            info!("deposit_addr_hub: {}", addr);
        }
        if let Some(ref addr) = s.withdraw_addr_kaspa {
            info!("withdraw_addr_kaspa: {}", addr);
        }

        // Show errors if any
        if let Some(ref error) = s.deposit_credit_error {
            info!("deposit_credit_error: {}", error);
        }
        if let Some(ref error) = s.withdraw_credit_error {
            info!("withdraw_credit_error: {}", error);
        }
    }
}

pub fn write_stats(file_path: &str, stats: Vec<RoundTripStats>, total_spend: u64, total_ops: u64) {
    let mut file = File::create(file_path).unwrap();
    serde_json::to_writer_pretty(&mut file, &stats).unwrap();
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RoundTripStats {
    pub op_id: u64,
    pub value: u64,
    pub kaspa_deposit_tx_id: Option<TransactionId>,
    #[serde(serialize_with = "serialize_systemtime_as_millis")]
    pub kaspa_deposit_tx_time: Option<SystemTime>,
    #[serde(serialize_with = "serialize_systemtime_as_millis")]
    pub deposit_credit_time: Option<SystemTime>,
    pub deposit_credit_error: Option<String>,
    pub hub_withdraw_tx_id: Option<TendermintHash>,
    #[serde(serialize_with = "serialize_systemtime_as_millis")]
    pub hub_withdraw_tx_time: Option<SystemTime>,
    #[serde(serialize_with = "serialize_systemtime_as_millis")]
    pub withdraw_credit_time: Option<SystemTime>,
    pub withdraw_credit_error: Option<String>,
    pub deposit_addr_hub: Option<String>,
    pub withdraw_addr_kaspa: Option<Address>,
}

#[derive(Debug, Clone, Copy)]
enum Stage {
    PreDeposit,
    PostDepositNotCredited,
    PreWithdrawal,
    PostWithdrawalNotCredited,
    Complete,
}

impl RoundTripStats {
    pub fn new(op_id: u64, value: u64) -> Self {
        let mut d = RoundTripStats::default();
        d.op_id = op_id;
        d.value = value;
        d
    }
    pub fn deposit_time(&self) -> Duration {
        // Time from deposit submission to hub credit
        self.deposit_credit_time
            .unwrap()
            .duration_since(self.kaspa_deposit_tx_time.unwrap())
            .unwrap()
    }
    pub fn withdraw_time(&self) -> Duration {
        // Time from withdrawal submission to kaspa credit
        self.withdraw_credit_time
            .unwrap()
            .duration_since(self.hub_withdraw_tx_time.unwrap())
            .unwrap()
    }
    pub fn stage(&self) -> Stage {
        if !self.kaspa_deposit_tx_time.is_some() {
            return Stage::PreDeposit;
        }
        if self.deposit_credit_error.is_some() {
            return Stage::PostDepositNotCredited;
        }
        if !self.hub_withdraw_tx_time.is_some() {
            return Stage::PreWithdrawal;
        }
        if self.withdraw_credit_error.is_some() {
            return Stage::PostWithdrawalNotCredited;
        }
        Stage::Complete
    }
}
