use eyre::Result;
use tracing::warn;

use hyperlane_core::{Encode, H256, H512};

use super::HyperlaneRocksDB;

const DELIVERY_TX: &str = "delivery_tx_";
const MESSAGE_ID_BY_TX: &str = "message_id_by_tx_";

/// Implementation of DeliveryDb for HyperlaneRocksDB
/// Tracks delivery transaction hashes for all destination chains
impl hyperlane_core::DeliveryDb for HyperlaneRocksDB {
    fn store_delivery_tx(&self, message_id: &H256, destination_tx: &H512) -> Result<()> {
        warn!(
            message_id = ?message_id,
            destination_tx = ?destination_tx,
            "DELIVERY_STORAGE: Storing delivery transaction"
        );
        // Store message_id -> tx_hash mapping
        match self.store_encodable(DELIVERY_TX, message_id.to_vec(), destination_tx) {
            Ok(()) => {
                // Also store reverse mapping: tx_hash -> message_id
                match self.store_encodable(MESSAGE_ID_BY_TX, destination_tx.to_vec(), message_id) {
                    Ok(()) => {
                        warn!(
                            message_id = ?message_id,
                            destination_tx = ?destination_tx,
                            "DELIVERY_STORAGE: Successfully stored delivery transaction (both directions)"
                        );
                        Ok(())
                    }
                    Err(e) => {
                        warn!(
                            message_id = ?message_id,
                            destination_tx = ?destination_tx,
                            error = %e,
                            "DELIVERY_STORAGE: Failed to store reverse mapping"
                        );
                        Err(e.into())
                    }
                }
            }
            Err(e) => {
                warn!(
                    message_id = ?message_id,
                    destination_tx = ?destination_tx,
                    error = %e,
                    "DELIVERY_STORAGE: Failed to store delivery transaction"
                );
                Err(e.into())
            }
        }
    }

    fn retrieve_delivery_tx(&self, message_id: &H256) -> Result<Option<H512>> {
        warn!(
            message_id = ?message_id,
            "DELIVERY_STORAGE: Retrieving delivery transaction"
        );
        match self.retrieve_decodable(DELIVERY_TX, message_id.to_vec()) {
            Ok(Some(tx)) => {
                warn!(
                    message_id = ?message_id,
                    tx_hash = ?tx,
                    "DELIVERY_STORAGE: Found delivery transaction"
                );
                Ok(Some(tx))
            }
            Ok(None) => {
                warn!(
                    message_id = ?message_id,
                    "DELIVERY_STORAGE: No delivery transaction found"
                );
                Ok(None)
            }
            Err(e) => {
                warn!(
                    message_id = ?message_id,
                    error = %e,
                    "DELIVERY_STORAGE: Error retrieving delivery transaction"
                );
                Err(e.into())
            }
        }
    }

    fn retrieve_message_id_by_tx(&self, destination_tx: &H512) -> Result<Option<H256>> {
        warn!(
            destination_tx = ?destination_tx,
            "DELIVERY_STORAGE: Retrieving message_id by transaction hash"
        );
        match self.retrieve_decodable(MESSAGE_ID_BY_TX, destination_tx.to_vec()) {
            Ok(Some(message_id)) => {
                warn!(
                    destination_tx = ?destination_tx,
                    message_id = ?message_id,
                    "DELIVERY_STORAGE: Found message_id for transaction hash"
                );
                Ok(Some(message_id))
            }
            Ok(None) => {
                warn!(
                    destination_tx = ?destination_tx,
                    "DELIVERY_STORAGE: No message_id found for transaction hash"
                );
                Ok(None)
            }
            Err(e) => {
                warn!(
                    destination_tx = ?destination_tx,
                    error = %e,
                    "DELIVERY_STORAGE: Error retrieving message_id by transaction hash"
                );
                Err(e.into())
            }
        }
    }
}

