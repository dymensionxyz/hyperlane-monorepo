use axum::{
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use hyperlane_base::server::utils::{
    ServerErrorBody, ServerErrorResponse, ServerResult, ServerSuccessResponse,
};
use hyperlane_core::{DeliveryDb, HyperlaneDomainProtocol, H256};

use bs58;

use crate::server::delivered::ServerState;

#[derive(Clone, Debug, Deserialize)]
pub struct QueryParams {
    /// The Hyperlane message ID (hex string, 64 characters, with or without 0x prefix)
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
    let message_id_str = &query_params.message_id;
    let domain_id = query_params.domain_id;

    let message_id: H256 = match message_id_str.parse() {
        Ok(id) => {
            debug!(%message_id_str, %domain_id, message_id = ?id, "parsed message_id");
            id
        }
        Err(e) => {
            debug!(
                %message_id_str,
                %domain_id,
                error = %e,
                "invalid message_id format"
            );
            return Err(ServerErrorResponse::new(
                StatusCode::BAD_REQUEST,
                ServerErrorBody {
                    message: format!(
                        "Invalid message_id format: {}. Expected 64 hex characters (32 bytes), with or without 0x prefix.",
                        e
                    ),
                },
            ));
        }
    };

    let db = match state.dbs.get(&domain_id) {
        Some(db) => db,
        None => {
            debug!(
                %message_id_str,
                %domain_id,
                available_domains = ?state.dbs.keys().collect::<Vec<_>>(),
                "no database found for domain"
            );
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

    let tx_hash = match db.retrieve_delivery_tx(&message_id) {
        Ok(Some(tx)) => {
            let domain = db.domain();
            let tx_hash_str = if domain.domain_protocol() == HyperlaneDomainProtocol::Sealevel {
                let base58_tx = bs58::encode(tx.as_bytes()).into_string();
                debug!(%message_id_str, %domain_id, tx_hash = %base58_tx, "found delivery tx (base58)");
                base58_tx
            } else {
                let hex_tx = format!("{:x}", tx);
                debug!(%message_id_str, %domain_id, tx_hash = %hex_tx, "found delivery tx (hex)");
                hex_tx
            };
            Some(tx_hash_str)
        }
        Ok(None) => {
            debug!(%message_id_str, %domain_id, "no delivery tx found");
            None
        }
        Err(e) => {
            error!(%message_id_str, %domain_id, error = %e, "database error retrieving delivery tx");
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
