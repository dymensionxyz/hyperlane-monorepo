use axum::{
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use hyperlane_base::{
    db::HyperlaneDb,
    server::utils::{
        ServerErrorBody, ServerErrorResponse, ServerResult, ServerSuccessResponse,
    },
};
use hyperlane_core::{HyperlaneDomainProtocol, H512};

use bs58;

use crate::server::delivered::ServerState;

/// Solana transaction signatures are 64 bytes
const SOLANA_SIGNATURE_BYTES: usize = 64;

#[derive(Clone, Debug, Deserialize)]
pub struct QueryParams {
    /// The transaction hash (base58 for Sealevel, hex for others)
    pub tx_hash: String,
    /// The domain ID (where the transaction hash is from)
    pub domain_id: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MessageIdResponse {
    /// The Hyperlane message ID
    pub message_id: String,
    /// The destination domain ID
    pub destination_domain_id: u32,
}

/// Retrieve the Hyperlane message ID for a given transaction hash (reverse lookup)
pub async fn handler(
    State(state): State<ServerState>,
    Query(query_params): Query<QueryParams>,
) -> ServerResult<ServerSuccessResponse<MessageIdResponse>> {
    let tx_hash_str = &query_params.tx_hash;
    let domain_id = query_params.domain_id;

    let db = match state.dbs.get(&domain_id) {
        Some(db) => db,
        None => {
            debug!(
                %tx_hash_str,
                %domain_id,
                available_domains = ?state.dbs.keys().collect::<Vec<_>>(),
                "no database found for origin domain"
            );
            return Err(ServerErrorResponse::new(
                StatusCode::NOT_FOUND,
                ServerErrorBody {
                    message: format!(
                        "No database found for origin domain: {}. Available domains: {:?}",
                        domain_id,
                        state.dbs.keys().collect::<Vec<_>>()
                    ),
                },
            ));
        }
    };

    let domain = db.domain();
    let is_sealevel = domain.domain_protocol() == HyperlaneDomainProtocol::Sealevel;

    let tx_hash_h512: H512 = if is_sealevel {
        debug!(%tx_hash_str, %domain_id, "parsing tx_hash as base58 (Sealevel)");
        match bs58::decode(tx_hash_str).into_vec() {
            Ok(bytes) => {
                if bytes.len() != SOLANA_SIGNATURE_BYTES {
                    debug!(
                        %tx_hash_str,
                        %domain_id,
                        bytes_len = %bytes.len(),
                        "invalid base58 tx_hash length"
                    );
                    return Err(ServerErrorResponse::new(
                        StatusCode::BAD_REQUEST,
                        ServerErrorBody {
                            message: format!(
                                "Invalid base58 tx_hash length: expected {} bytes, got {}",
                                SOLANA_SIGNATURE_BYTES,
                                bytes.len()
                            ),
                        },
                    ));
                }
                H512::from_slice(&bytes)
            }
            Err(e) => {
                debug!(%tx_hash_str, %domain_id, error = %e, "failed to parse base58 tx_hash");
                return Err(ServerErrorResponse::new(
                    StatusCode::BAD_REQUEST,
                    ServerErrorBody {
                        message: format!("Invalid base58 tx_hash format: {}", e),
                    },
                ));
            }
        }
    } else {
        match tx_hash_str.parse() {
            Ok(hash) => hash,
            Err(e) => {
                debug!(%tx_hash_str, %domain_id, error = %e, "failed to parse hex tx_hash");
                return Err(ServerErrorResponse::new(
                    StatusCode::BAD_REQUEST,
                    ServerErrorBody {
                        message: format!(
                            "Invalid hex tx_hash format: {}. Expected 128 hex characters (64 bytes), with or without 0x prefix",
                            e
                        ),
                    },
                ));
            }
        }
    };

    let message_id = match db.retrieve_message_id_by_dispatch_tx(&tx_hash_h512) {
        Ok(Some(message_id)) => {
            debug!(%tx_hash_str, %domain_id, message_id = ?message_id, "found message_id");
            message_id
        }
        Ok(None) => {
            debug!(%tx_hash_str, %domain_id, "no message_id found");
            return Err(ServerErrorResponse::new(
                StatusCode::NOT_FOUND,
                ServerErrorBody {
                    message: format!(
                        "No message found for tx_hash: {} on origin domain: {}",
                        tx_hash_str, domain_id
                    ),
                },
            ));
        }
        Err(e) => {
            error!(%tx_hash_str, %domain_id, error = %e, "database error retrieving message_id");
            return Err(ServerErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                ServerErrorBody {
                    message: format!("Database error: {}", e),
                },
            ));
        }
    };

    let message = match db.retrieve_message_by_id(&message_id) {
        Ok(Some(message)) => {
            debug!(
                %tx_hash_str,
                %domain_id,
                message_id = ?message_id,
                destination_domain_id = %message.destination,
                "retrieved message"
            );
            message
        }
        Ok(None) => {
            error!(
                %tx_hash_str,
                %domain_id,
                message_id = ?message_id,
                "message_id found but full message missing"
            );
            return Err(ServerErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                ServerErrorBody {
                    message: "Message ID found but message data missing from database".to_string(),
                },
            ));
        }
        Err(e) => {
            error!(
                %tx_hash_str,
                %domain_id,
                message_id = ?message_id,
                error = %e,
                "database error retrieving message"
            );
            return Err(ServerErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                ServerErrorBody {
                    message: format!("Database error: {}", e),
                },
            ));
        }
    };

    let response = MessageIdResponse {
        message_id: format!("{:x}", message_id),
        destination_domain_id: message.destination,
    };

    Ok(ServerSuccessResponse::new(response))
}
