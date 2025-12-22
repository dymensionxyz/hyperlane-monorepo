use eyre::Result;
use tracing::debug;

use hyperlane_core::{Encode, H256, H512};

use super::HyperlaneRocksDB;

const DELIVERY_TX: &str = "delivery_tx_";

/// Implementation of DeliveryDb for HyperlaneRocksDB
/// Tracks delivery transaction hashes for all destination chains
impl hyperlane_core::DeliveryDb for HyperlaneRocksDB {
    fn store_delivery_tx(&self, message_id: &H256, destination_tx: &H512) -> Result<()> {
        debug!(
            message_id = ?message_id,
            destination_tx = ?destination_tx,
            "Storing delivery transaction"
        );
        self.store_encodable(DELIVERY_TX, message_id.to_vec(), destination_tx)?;
        Ok(())
    }

    fn retrieve_delivery_tx(&self, message_id: &H256) -> Result<Option<H512>> {
        Ok(self.retrieve_decodable(DELIVERY_TX, message_id.to_vec())?)
    }
}

