use eyre::Result;
use tracing::{debug, error};

use hyperlane_core::{Encode, H256, H512};

use super::HyperlaneRocksDB;

const DELIVERY_TX: &str = "delivery_tx_";
const MESSAGE_ID_BY_TX: &str = "message_id_by_tx_";

/// Implementation of DeliveryDb for HyperlaneRocksDB
/// Tracks delivery transaction hashes for all destination chains
impl hyperlane_core::DeliveryDb for HyperlaneRocksDB {
    fn store_delivery_tx(&self, message_id: &H256, destination_tx: &H512) -> Result<()> {
        self.store_encodable(DELIVERY_TX, message_id.to_vec(), destination_tx)?;
        self.store_encodable(MESSAGE_ID_BY_TX, destination_tx.to_vec(), message_id)?;
        debug!(message_id = ?message_id, destination_tx = ?destination_tx, "stored delivery tx");
        Ok(())
    }

    fn retrieve_delivery_tx(&self, message_id: &H256) -> Result<Option<H512>> {
        match self.retrieve_decodable(DELIVERY_TX, message_id.to_vec()) {
            Ok(Some(tx)) => {
                debug!(message_id = ?message_id, tx_hash = ?tx, "found delivery tx");
                Ok(Some(tx))
            }
            Ok(None) => {
                debug!(message_id = ?message_id, "no delivery tx found");
                Ok(None)
            }
            Err(e) => {
                error!(message_id = ?message_id, error = %e, "error retrieving delivery tx");
                Err(e.into())
            }
        }
    }
}
