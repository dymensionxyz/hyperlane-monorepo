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
    pub nonce_start: u32,
    pub nonce_end: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct WithdrawalResponse {
    pub message_id: String,
    pub message: HyperlaneMessage,
    pub nonce: u32,
    pub processed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResponseBody {
    pub withdrawals: Vec<WithdrawalResponse>,
}

/// Fetch Kaspa withdrawals from the database
pub async fn handler(
    State(state): State<ServerState>,
    Query(query_params): Query<QueryParams>,
) -> ServerResult<ServerSuccessResponse<ResponseBody>> {
    let QueryParams {
        nonce_start,
        nonce_end,
    } = query_params;

    tracing::debug!(nonce_start, nonce_end, "Fetching Kaspa withdrawals");

    if nonce_end <= nonce_start {
        let error_msg = "nonce_end must be greater than nonce_start";
        let err = ServerErrorResponse::new(
            StatusCode::BAD_REQUEST,
            ServerErrorBody {
                message: error_msg.to_string(),
            },
        );
        return Err(err);
    }

    let db = &state.kaspa_db;
    let mut withdrawals = Vec::new();

    // Iterate through the nonce range and fetch withdrawal messages
    for nonce in nonce_start..nonce_end {
        match db.as_ref().retrieve_kaspa_withdrawal_by_nonce(nonce) {
            Ok(Some(message)) => {
                // Check if the message has been processed
                let processed = db
                    .as_ref()
                    .retrieve_processed_by_nonce(&nonce)
                    .unwrap_or(Some(false))
                    .unwrap_or(false);

                withdrawals.push(WithdrawalResponse {
                    message_id: format!("{:x}", message.id()),
                    nonce,
                    message,
                    processed,
                });
            }
            Ok(None) => {
                // No message at this nonce, continue
                tracing::trace!(nonce, "No withdrawal found at nonce");
            }
            Err(e) => {
                tracing::error!(nonce, error = ?e, "Error retrieving withdrawal from database");
                return Err(ServerErrorResponse::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ServerErrorBody {
                        message: format!("Database error: {}", e),
                    },
                ));
            }
        }
    }

    let resp = ResponseBody { withdrawals };
    Ok(ServerSuccessResponse::new(resp))
}
