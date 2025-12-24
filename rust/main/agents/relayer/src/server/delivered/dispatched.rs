use axum::{
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::warn;

use hyperlane_base::{
    db::HyperlaneDb,
    server::utils::{
        ServerErrorBody, ServerErrorResponse, ServerResult, ServerSuccessResponse,
    },
};
use hyperlane_core::{HyperlaneDomainProtocol, H512};

// For parsing base58 transaction signatures for Solana
use bs58;

use crate::server::delivered::ServerState;

#[derive(Clone, Debug, Deserialize)]
pub struct QueryParams {
    /// The transaction hash (base58 for Sealevel, hex for others)
    /// For Sealevel: base58 string, e.g. "kKe43MZtkjypsbgwKvrCVZWNmsYFm2aqTUyWzHPEAqWq5f3kwegKKjbPpjsP8MvcTRzbgZ1mg4sfqxRcwJGZ2ZD"
    /// For others: hex string (with or without 0x prefix), e.g. "0xabc123..."
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
        Some(db) => {
            db
        }
        None => {
            warn!(
                %tx_hash_str,
                %domain_id,
                available_domains = ?state.dbs.keys().collect::<Vec<_>>(),
                "DISPATCHED_API: No database found for origin domain"
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

    // Get the domain to determine the protocol
    let domain = db.domain();
    let is_sealevel = domain.domain_protocol() == HyperlaneDomainProtocol::Sealevel;

    // Parse the tx_hash based on the domain protocol
    let tx_hash_h512: H512 = if is_sealevel {
        // For Sealevel, parse as base58
        warn!(
            %tx_hash_str,
            %domain_id,
            "DISPATCHED_API: Parsing tx_hash as base58 (Sealevel)"
        );
        match bs58::decode(&tx_hash_str).into_vec() {
            Ok(bytes) => {
                if bytes.len() != 64 {
                    warn!(
                        %tx_hash_str,
                        %domain_id,
                        bytes_len = %bytes.len(),
                        "DISPATCHED_API: Invalid base58 tx_hash length - expected 64 bytes"
                    );
                    return Err(ServerErrorResponse::new(
                        StatusCode::BAD_REQUEST,
                        ServerErrorBody {
                            message: format!(
                                "Invalid base58 tx_hash length: expected 64 bytes, got {}",
                                bytes.len()
                            ),
                        },
                    ));
                }
                H512::from_slice(&bytes)
            }
            Err(e) => {
                warn!(
                    %tx_hash_str,
                    %domain_id,
                    error = %e,
                    "DISPATCHED_API: Failed to parse base58 tx_hash"
                );
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
        match tx_hash_str.parse() {
            Ok(hash) => hash,
            Err(e) => {
                warn!(
                    %tx_hash_str,
                    %domain_id,
                    error = %e,
                    "DISPATCHED_API: Failed to parse hex tx_hash"
                );
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

    // Retrieve from database (where relayer stores dispatch tx -> message_id mappings)
    let message_id = match db.retrieve_message_id_by_dispatch_tx(&tx_hash_h512) {
        Ok(Some(message_id)) => {
            warn!(
                %tx_hash_str,
                %domain_id,
                message_id = ?message_id,
                "DISPATCHED_API: Found message_id in database"
            );
            message_id
        }
        Ok(None) => {
            warn!(
                %tx_hash_str,
                %domain_id,
                "DISPATCHED_API: No message_id found in database"
            );
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
            warn!(
                %tx_hash_str,
                %domain_id,
                error = %e,
                "DISPATCHED_API: Database error when retrieving message_id"
            );
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
        Ok(Some(message)) => {
            warn!(
                %tx_hash_str,
                %domain_id,
                message_id = ?message_id,
                destination_domain_id = %message.destination,
                "DISPATCHED_API: Successfully retrieved message from database"
            );
            message
        }
        Ok(None) => {
            warn!(
                %tx_hash_str,
                %domain_id,
                message_id = ?message_id,
                "DISPATCHED_API: message_id found in database but full message not found"
            );
            return Err(ServerErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                ServerErrorBody {
                    message: "Message ID found but message data missing from database".to_string(),
                },
            ));
        }
        Err(e) => {
            warn!(
                %tx_hash_str,
                %domain_id,
                message_id = ?message_id,
                error = %e,
                "DISPATCHED_API: Database error retrieving full message"
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

