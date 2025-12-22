use eyre::Result;
use tracing::debug;

use hyperlane_core::{Encode, H256, H512};

use super::HyperlaneRocksDB;

const SEALEVEL_DELIVERY_TX: &str = "sealevel_delivery_tx_";

/// Implementation of SealevelDb for HyperlaneRocksDB
impl hyperlane_core::SealevelDb for HyperlaneRocksDB {
    fn store_delivery_tx(&self, message_id: &H256, destination_tx: &H512) -> Result<()> {
        debug!(
            message_id = ?message_id,
            destination_tx = ?destination_tx,
            "Storing Sealevel delivery transaction"
        );
        self.store_encodable(SEALEVEL_DELIVERY_TX, message_id.to_vec(), destination_tx)?;
        Ok(())
    }

    fn retrieve_delivery_tx(&self, message_id: &H256) -> Result<Option<H512>> {
        Ok(self.retrieve_decodable(SEALEVEL_DELIVERY_TX, message_id.to_vec())?)
    }
}

