use axum::{
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use hyperlane_base::db::HyperlaneDb;
use hyperlane_base::server::utils::{
    ServerErrorBody, ServerErrorResponse, ServerResult, ServerSuccessResponse,
};
use hyperlane_core::HyperlaneMessage;

use crate::server::kaspa::ServerState;

#[derive(Clone, Debug, Deserialize)]
pub struct QueryParams {
    pub kaspa_tx: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct DepositResponse {
    pub message_id: String,
    pub message: HyperlaneMessage,
    pub kaspa_tx: String,
    pub status: String,
}

/// Fetch a Kaspa deposit by kaspa transaction hash
pub async fn handler(
    State(state): State<ServerState>,
    Query(query_params): Query<QueryParams>,
) -> ServerResult<ServerSuccessResponse<DepositResponse>> {
    let kaspa_tx = query_params.kaspa_tx;

    tracing::debug!(%kaspa_tx, "Fetching Kaspa deposit by kaspa_tx");

    let db = &state.kaspa_db;

    // Retrieve the deposit message directly by tx_hash
    let message = match db.as_ref().retrieve_kaspa_deposit_by_tx_hash(&kaspa_tx) {
        Ok(Some(message)) => message,
        Ok(None) => {
            return Err(ServerErrorResponse::new(
                StatusCode::NOT_FOUND,
                ServerErrorBody {
                    message: format!("No deposit found for kaspa_tx: {}", kaspa_tx),
                },
            ));
        }
        Err(e) => {
            tracing::error!(%kaspa_tx, error = ?e, "Error retrieving deposit from database");
            return Err(ServerErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                ServerErrorBody {
                    message: format!("Database error: {}", e),
                },
            ));
        }
    };

    let message_id = message.id();

    // Determine status: check if message has been processed on Hub
    let status = if db.as_ref().retrieve_processed_by_nonce(&message.nonce)
        .unwrap_or(Some(false))
        .unwrap_or(false)
    {
        "completed".to_string()
    } else {
        "pending".to_string()
    };

    let response = DepositResponse {
        message_id: format!("{:x}", message_id),
        message,
        kaspa_tx,
        status,
    };

    Ok(ServerSuccessResponse::new(response))
}
