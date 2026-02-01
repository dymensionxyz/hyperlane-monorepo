use axum::{
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn, error};

use hyperlane_base::server::utils::{
    ServerErrorBody, ServerErrorResponse, ServerResult, ServerSuccessResponse,
};
use hyperlane_core::{h512_to_bytes, DeliveryDb, HyperlaneDomainProtocol, H256};

// For converting H512 to base58 for Solana transaction signatures
use bs58;

use crate::server::delivered::ServerState;

#[derive(Clone, Debug, Deserialize)]
pub struct QueryParams {
    /// The Hyperlane message ID (hex string, 64 characters, with or without 0x prefix)
    /// Example: "0x8ebdc20c6c728c5715412ee928599c7286151f76d9079c8bdee08a335c7d072f"
    /// or: "8ebdc20c6c728c5715412ee928599c7286151f76d9079c8bdee08a335c7d072f"
    pub message_id: String,
    /// The destination domain ID
    pub domain_id: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct DeliveredResponse {
    /// Whether the message has been delivered
    pub delivered: bool,
    /// The destination transaction hash (if delivered)
    pub tx_hash: Option<String>,
}

/// Check if a message has been delivered to a destination and return the tx hash
pub async fn handler(
    State(state): State<ServerState>,
    Query(query_params): Query<QueryParams>,
) -> ServerResult<ServerSuccessResponse<DeliveredResponse>> {    
    let message_id_str = query_params.message_id.clone();
    let domain_id = query_params.domain_id;

    // Parse the message ID (accepts hex with or without 0x prefix)
    // Expected format: 64 hex characters (32 bytes), e.g. "0x8ebdc20c6c728c5715412ee928599c7286151f76d9079c8bdee08a335c7d072f"
    let message_id: H256 = match message_id_str.parse() {
        Ok(id) => id,
        Err(e) => {
            return Err(ServerErrorResponse::new(
                StatusCode::BAD_REQUEST,
                ServerErrorBody {
                    message: format!(
                        "Invalid message_id format: {}. Expected 64 hex characters (32 bytes), with or without 0x prefix.",
                        e
                    ),
                },
            ))
        }
    };

    let db = match state.dbs.get(&domain_id) {
        Some(db) => db,
        None => {
            return Err(ServerErrorResponse::new(
                StatusCode::NOT_FOUND,
                ServerErrorBody {
                    message: format!(
                        "No database found for domain: {}. Available domains: {:?}",
                        domain_id,
                        state.dbs.keys().collect::<Vec<_>>()
                    ),
                },
            ));
        }
    };

    // Retrieve the delivery tx hash from the database
    let tx_hash = match db.retrieve_delivery_tx(&message_id) {
        Ok(Some(tx)) => {
            // Check if this is a Sealevel domain - if so, convert H512 to base58
            // Otherwise, return as hex
            let domain = db.domain();
            let tx_hash_str = if domain.domain_protocol() == HyperlaneDomainProtocol::Sealevel {
                // Convert H512 to base58 for Solana transaction signatures
                let base58_tx = bs58::encode(tx.as_bytes()).into_string();
                base58_tx
            } else {
                // For other chains (like Ethereum), convert H512 to bytes intelligently
                // h512_to_bytes will extract the last 32 bytes if the first 32 bytes are zeros
                // This handles the case where Ethereum tx hashes (H256) are stored as H512
                let tx_bytes = h512_to_bytes(&tx);
                
                // Convert bytes to hex string manually
                let mut hex_tx = String::with_capacity(2 + tx_bytes.len() * 2);
                hex_tx.push_str("0x");
                for byte in tx_bytes.iter() {
                    hex_tx.push_str(&format!("{:02x}", byte));
                }
                hex_tx
            };
            Some(tx_hash_str)
        }
        Ok(None) => {
            None
        }
        Err(e) => {
            return Err(ServerErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                ServerErrorBody {
                    message: format!("Database error: {}", e),
                },
            ));
        }
    };

    let delivered = tx_hash.is_some();
    let response = DeliveredResponse { delivered, tx_hash };

    Ok(ServerSuccessResponse::new(response))
}
