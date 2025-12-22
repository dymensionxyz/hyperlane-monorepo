use axum::{
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::warn;

use hyperlane_base::server::utils::{
    ServerErrorBody, ServerErrorResponse, ServerResult, ServerSuccessResponse,
};
use hyperlane_core::{DeliveryDb, H256};

use crate::server::delivered::ServerState;

#[derive(Clone, Debug, Deserialize)]
pub struct QueryParams {
    /// The Hyperlane message ID
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
    warn!("DELIVERY_API: Handler called");
    
    let message_id_str = query_params.message_id.clone();
    let domain_id = query_params.domain_id;

    warn!(
        %message_id_str,
        %domain_id,
        "DELIVERY_API: Checking delivery status"
    );

    // Parse the message ID
    let message_id: H256 = match message_id_str.parse() {
        Ok(id) => {
            warn!(
                %message_id_str,
                %domain_id,
                message_id = ?id,
                "DELIVERY_API: Successfully parsed message_id"
            );
            id
        }
        Err(e) => {
            warn!(
                %message_id_str,
                %domain_id,
                error = %e,
                "DELIVERY_API: Invalid message_id format"
            );
            return Err(ServerErrorResponse::new(
                StatusCode::BAD_REQUEST,
                ServerErrorBody {
                    message: format!("Invalid message_id format: {}", e),
                },
            ));
        }
    };

    // Get the database for the destination domain
    let db = match state.dbs.get(&domain_id) {
        Some(db) => {
            warn!(
                %message_id_str,
                %domain_id,
                "DELIVERY_API: Found database for domain"
            );
            db
        }
        None => {
            warn!(
                %message_id_str,
                %domain_id,
                available_domains = ?state.dbs.keys().collect::<Vec<_>>(),
                "DELIVERY_API: No database found for domain"
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

    // Retrieve the delivery tx hash from the database
    let tx_hash = match db.retrieve_delivery_tx(&message_id) {
        Ok(Some(tx)) => {
            warn!(
                %message_id_str,
                %domain_id,
                tx_hash = %format!("{:x}", tx),
                "DELIVERY_API: Found delivery tx hash in database"
            );
            Some(format!("{:x}", tx))
        }
        Ok(None) => {
            warn!(
                %message_id_str,
                %domain_id,
                "DELIVERY_API: No delivery tx hash found in database (message not delivered or not stored)"
            );
            None
        }
        Err(e) => {
            warn!(
                %message_id_str,
                %domain_id,
                error = %e,
                "DELIVERY_API: Error retrieving delivery tx from database"
            );
            return Err(ServerErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                ServerErrorBody {
                    message: format!("Database error: {}", e),
                },
            ));
        }
    };

    let delivered = tx_hash.is_some();

    warn!(
        %message_id_str,
        %domain_id,
        delivered = %delivered,
        tx_hash = ?tx_hash,
        "DELIVERY_API: Returning response"
    );

    let response = DeliveredResponse { delivered, tx_hash };

    Ok(ServerSuccessResponse::new(response))
}

