use axum::{
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use hyperlane_base::{
    db::HyperlaneDb,
    server::utils::{
        ServerErrorBody, ServerErrorResponse, ServerResult, ServerSuccessResponse,
    },
};
use hyperlane_core::{HyperlaneDomainProtocol, H256, H512};

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
    let tx_hash_str = query_params.tx_hash.clone();
    let domain_id = query_params.domain_id;

    // Get the database for the domain (where the tx hash is from)
    let db = match state.dbs.get(&domain_id) {
        Some(db) => db,
        None => {
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

    // Get the domain to determine the protocol
    let domain = db.domain();
    let is_sealevel = domain.domain_protocol() == HyperlaneDomainProtocol::Sealevel;

    // Parse the tx_hash based on the domain protocol
    let tx_hash_h512: H512 = if is_sealevel {
        match bs58::decode(&tx_hash_str).into_vec() {
            Ok(bytes) => {
                if bytes.len() != SOLANA_SIGNATURE_BYTES {
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
                return Err(ServerErrorResponse::new(
                    StatusCode::BAD_REQUEST,
                    ServerErrorBody {
                        message: format!("Invalid base58 tx_hash format: {}", e),
                    },
                ));
            }
        }
    } else {
        // For other chains, parse as hex
        // Accept both H256 (64 hex chars / 32 bytes) and H512 (128 hex chars / 64 bytes)
        let tx_hash_without_prefix = tx_hash_str.strip_prefix("0x").unwrap_or(&tx_hash_str);
        let hex_len = tx_hash_without_prefix.len();
        
        if hex_len == 64 {
            // H256 format (32 bytes / 64 hex chars) - convert to H512
            match tx_hash_str.parse::<H256>() {
                Ok(hash_h256) => {
                    let hash_h512: H512 = hash_h256.into();
                    hash_h512
                }
                Err(e) => {
                    return Err(ServerErrorResponse::new(
                        StatusCode::BAD_REQUEST,
                        ServerErrorBody {
                            message: format!(
                                "Invalid hex tx_hash format: {}. Expected 64 hex characters (32 bytes) or 128 hex characters (64 bytes), with or without 0x prefix",
                                e
                            ),
                        },
                    ));
                }
            }
        } else if hex_len == 128 {
            // H512 format (64 bytes / 128 hex chars)
            match tx_hash_str.parse::<H512>() {
                Ok(hash) => hash,
                Err(e) => {
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
        } else {
            return Err(ServerErrorResponse::new(
                StatusCode::BAD_REQUEST,
                ServerErrorBody {
                    message: format!(
                        "Invalid tx_hash length: expected 64 hex characters (32 bytes) or 128 hex characters (64 bytes), got {} characters",
                        hex_len
                    ),
                },
            ));
        }
    };

    // Retrieve from database (where relayer stores dispatch tx -> message_id mappings)
    let message_id = match db.retrieve_message_id_by_dispatch_tx(&tx_hash_h512) {
        Ok(Some(message_id)) => message_id,
        Ok(None) => {
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

    // Get the full message to extract destination_domain_id
    let message = match db.retrieve_message_by_id(&message_id) {
        Ok(Some(message)) => message,
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
