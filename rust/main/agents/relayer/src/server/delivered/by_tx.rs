use axum::{
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::warn;

use hyperlane_base::server::utils::{
    ServerErrorBody, ServerErrorResponse, ServerResult, ServerSuccessResponse,
};
use hyperlane_core::{HyperlaneDomainProtocol, HyperlaneMessage, H512};

// For parsing base58 transaction signatures for Solana
use bs58;

use crate::server::delivered::ServerState;

#[derive(Clone, Debug, Deserialize)]
pub struct QueryParams {
    /// The transaction hash (base58 for Sealevel, hex for others)
    /// For Sealevel: base58 string, e.g. "kKe43MZtkjypsbgwKvrCVZWNmsYFm2aqTUyWzHPEAqWq5f3kwegKKjbPpjsP8MvcTRzbgZ1mg4sfqxRcwJGZ2ZD"
    /// For others: hex string (with or without 0x prefix), e.g. "0xabc123..."
    pub tx_hash: String,
    /// The origin domain ID (where the transaction hash is from)
    pub origin_domain_id: u32,
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
    warn!("DELIVERY_API_BY_TX: Handler called");

    let tx_hash_str = query_params.tx_hash.clone();
    let origin_domain_id = query_params.origin_domain_id;

    warn!(
        %tx_hash_str,
        %origin_domain_id,
        "DELIVERY_API_BY_TX: Looking up message_id by tx_hash on origin domain"
    );

    // Get the database for the origin domain (where the tx hash is from)
    let db = match state.dbs.get(&origin_domain_id) {
        Some(db) => {
            warn!(
                %tx_hash_str,
                %origin_domain_id,
                "DELIVERY_API_BY_TX: Found database for origin domain"
            );
            db
        }
        None => {
            warn!(
                %tx_hash_str,
                %origin_domain_id,
                available_domains = ?state.dbs.keys().collect::<Vec<_>>(),
                "DELIVERY_API_BY_TX: No database found for origin domain"
            );
            return Err(ServerErrorResponse::new(
                StatusCode::NOT_FOUND,
                ServerErrorBody {
                    message: format!(
                        "No database found for origin domain: {}. Available domains: {:?}",
                        origin_domain_id,
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
            %origin_domain_id,
            "DELIVERY_API_BY_TX: Parsing tx_hash as base58 (Sealevel)"
        );
        match bs58::decode(&tx_hash_str).into_vec() {
            Ok(bytes) => {
                if bytes.len() != 64 {
                    warn!(
                        %tx_hash_str,
                        %origin_domain_id,
                        bytes_len = %bytes.len(),
                        "DELIVERY_API_BY_TX: Invalid base58 tx_hash length - expected 64 bytes"
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
                    %origin_domain_id,
                    error = %e,
                    "DELIVERY_API_BY_TX: Failed to parse base58 tx_hash"
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
        warn!(
            %tx_hash_str,
            %origin_domain_id,
            "DELIVERY_API_BY_TX: Parsing tx_hash as hex (non-Sealevel)"
        );
        match tx_hash_str.parse() {
            Ok(hash) => hash,
            Err(e) => {
                warn!(
                    %tx_hash_str,
                    %origin_domain_id,
                    error = %e,
                    "DELIVERY_API_BY_TX: Failed to parse hex tx_hash"
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

    warn!(
        %tx_hash_str,
        %origin_domain_id,
        tx_hash_h512 = ?tx_hash_h512,
        "DELIVERY_API_BY_TX: Successfully parsed tx_hash, checking database first"
    );

    // First, try to retrieve from database (where relayer stores dispatch tx -> message_id mappings)
    warn!(
        %tx_hash_str,
        %origin_domain_id,
        "DELIVERY_API_BY_TX: Checking database for dispatch tx -> message_id mapping"
    );

    let message_id_from_db = match db.retrieve_message_id_by_dispatch_tx(&tx_hash_h512) {
        Ok(Some(message_id)) => {
            warn!(
                %tx_hash_str,
                %origin_domain_id,
                message_id = ?message_id,
                "DELIVERY_API_BY_TX: Found message_id in database"
            );
            Some(message_id)
        }
        Ok(None) => {
            warn!(
                %tx_hash_str,
                %origin_domain_id,
                "DELIVERY_API_BY_TX: No message_id found in database, will query chain"
            );
            None
        }
        Err(e) => {
            warn!(
                %tx_hash_str,
                %origin_domain_id,
                error = %e,
                "DELIVERY_API_BY_TX: Database error when retrieving message_id, will query chain"
            );
            None
        }
    };

    // If we found the message_id in database, get the full message to extract destination_domain_id
    if let Some(message_id) = message_id_from_db {
        match db.retrieve_message_by_id(&message_id) {
            Ok(Some(message)) => {
                let destination_domain_id = message.destination;
                warn!(
                    %tx_hash_str,
                    %origin_domain_id,
                    message_id = ?message_id,
                    destination_domain_id = %destination_domain_id,
                    "DELIVERY_API_BY_TX: Successfully retrieved message from database, returning response"
                );

                let response = MessageIdResponse {
                    message_id: format!("{:x}", message_id),
                    destination_domain_id,
                };

                return Ok(ServerSuccessResponse::new(response));
            }
            Ok(None) => {
                warn!(
                    %tx_hash_str,
                    %origin_domain_id,
                    message_id = ?message_id,
                    "DELIVERY_API_BY_TX: message_id found in database but full message not found, will query chain"
                );
            }
            Err(e) => {
                warn!(
                    %tx_hash_str,
                    %origin_domain_id,
                    message_id = ?message_id,
                    error = %e,
                    "DELIVERY_API_BY_TX: Database error retrieving full message, will query chain"
                );
            }
        }
    }

    // Fallback: Query the chain for dispatch events by tx hash
    warn!(
        %tx_hash_str,
        %origin_domain_id,
        "DELIVERY_API_BY_TX: Querying chain for dispatch events"
    );

    let message_sync = match state.message_syncs.get(&origin_domain_id) {
        Some(sync) => {
            warn!(
                %tx_hash_str,
                %origin_domain_id,
                "DELIVERY_API_BY_TX: Found message_sync for origin domain"
            );
            sync
        }
        None => {
            warn!(
                %tx_hash_str,
                %origin_domain_id,
                available_domains = ?state.message_syncs.keys().collect::<Vec<_>>(),
                "DELIVERY_API_BY_TX: No message_sync found for origin domain"
            );
            return Err(ServerErrorResponse::new(
                StatusCode::NOT_FOUND,
                ServerErrorBody {
                    message: format!(
                        "No message_sync available for origin domain: {}. Chain query not possible.",
                        origin_domain_id
                    ),
                },
            ));
        }
    };

    // Fetch dispatch events from the chain
    let dispatch_events = match message_sync.fetch_logs_by_tx_hash(tx_hash_h512).await {
        Ok(events) => {
            warn!(
                %tx_hash_str,
                %origin_domain_id,
                events_count = %events.len(),
                "DELIVERY_API_BY_TX: Fetched dispatch events from chain"
            );
            events
        }
        Err(e) => {
            warn!(
                %tx_hash_str,
                %origin_domain_id,
                error = %e,
                "DELIVERY_API_BY_TX: Error querying chain for dispatch events"
            );
            return Err(ServerErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                ServerErrorBody {
                    message: format!("Chain query error: {}", e),
                },
            ));
        }
    };

    // Extract the first message from the dispatch events
    let (indexed_message, _log_meta) = match dispatch_events.first() {
        Some(event) => event,
        None => {
            warn!(
                %tx_hash_str,
                %origin_domain_id,
                "DELIVERY_API_BY_TX: No dispatch events found for tx_hash"
            );
            return Err(ServerErrorResponse::new(
                StatusCode::NOT_FOUND,
                ServerErrorBody {
                    message: format!(
                        "No dispatch events found for tx_hash: {} on origin domain: {}",
                        tx_hash_str, origin_domain_id
                    ),
                },
            ));
        }
    };

    let message = indexed_message.inner();
    let message_id = message.id();
    let destination_domain_id = message.destination;

    warn!(
        %tx_hash_str,
        %origin_domain_id,
        message_id = ?message_id,
        destination_domain_id = %destination_domain_id,
        "DELIVERY_API_BY_TX: Found message in dispatch events"
    );

    warn!(
        %tx_hash_str,
        %origin_domain_id,
        message_id = ?message_id,
        destination_domain_id = %destination_domain_id,
        "DELIVERY_API_BY_TX: Successfully retrieved message from chain, returning response"
    );

    let response = MessageIdResponse {
        message_id: format!("{:x}", message_id),
        destination_domain_id,
    };

    Ok(ServerSuccessResponse::new(response))
}

