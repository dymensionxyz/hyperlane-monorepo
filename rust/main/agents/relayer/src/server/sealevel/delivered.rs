use axum::{
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use hyperlane_base::server::utils::{
    ServerErrorBody, ServerErrorResponse, ServerResult, ServerSuccessResponse,
};
use hyperlane_core::{SealevelDb, H256};

use crate::server::sealevel::ServerState;

#[derive(Clone, Debug, Deserialize)]
pub struct QueryParams {
    /// The Hyperlane message ID
    pub message_id: String,
    /// The destination domain ID (Sealevel chain)
    pub destination_domain: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct DeliveredResponse {
    /// Whether the message has been delivered
    pub delivered: bool,
    /// The destination transaction hash (if delivered)
    pub tx_hash: Option<String>,
}

/// Check if a message has been delivered to a Sealevel destination and return the tx hash
pub async fn handler(
    State(state): State<ServerState>,
    Query(query_params): Query<QueryParams>,
) -> ServerResult<ServerSuccessResponse<DeliveredResponse>> {
    let message_id_str = query_params.message_id;
    let destination_domain = query_params.destination_domain;

    tracing::debug!(
        %message_id_str,
        %destination_domain,
        "Checking Sealevel delivery status"
    );

    // Parse the message ID
    let message_id: H256 = message_id_str.parse().map_err(|e| {
        ServerErrorResponse::new(
            StatusCode::BAD_REQUEST,
            ServerErrorBody {
                message: format!("Invalid message_id format: {}", e),
            },
        )
    })?;

    // Get the database for the destination domain
    let db = state.dbs.get(&destination_domain).ok_or_else(|| {
        ServerErrorResponse::new(
            StatusCode::NOT_FOUND,
            ServerErrorBody {
                message: format!(
                    "No database found for destination domain: {}",
                    destination_domain
                ),
            },
        )
    })?;

    // Retrieve the delivery tx hash from the database
    let tx_hash = match db.retrieve_delivery_tx(&message_id) {
        Ok(Some(tx)) => Some(format!("{:x}", tx)),
        Ok(None) => None,
        Err(e) => {
            tracing::error!(
                %message_id_str,
                %destination_domain,
                error = ?e,
                "Error retrieving delivery tx from database"
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

    let response = DeliveredResponse { delivered, tx_hash };

    Ok(ServerSuccessResponse::new(response))
}

