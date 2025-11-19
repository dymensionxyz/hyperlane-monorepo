/// Trait for interacting with the Hub chain
/// This abstraction allows the bridge to work with any Hub implementation
use async_trait::async_trait;
use eyre::Result;
use kaspa_core::bridge_types::DepositResult;

#[async_trait]
pub trait HubClient: Send + Sync {
    /// Check if a message has been delivered on the Hub chain
    async fn query_message_delivered(&self, message_id: &[u8; 32]) -> Result<bool>;

    /// Submit a deposit to the Hub chain
    /// Returns the Hub transaction hash as hex string
    async fn submit_deposit(&self, deposit: &DepositResult) -> Result<String>;
}
