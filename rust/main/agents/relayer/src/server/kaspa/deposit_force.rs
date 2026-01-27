use axum::{extract::State, http::StatusCode, Json};
use dymension_kaspa::dym_kas_core::api::client::Deposit;
use serde::{Deserialize, Serialize};

use hyperlane_base::server::utils::{
    ServerErrorBody, ServerErrorResponse, ServerResult, ServerSuccessResponse,
};

use super::ServerState;

#[derive(Clone, Debug, Deserialize)]
pub struct RequestBody {
    pub kaspa_tx: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResponseBody {
    pub message: String,
    pub deposit_id: String,
}

/// Force processing of a Kaspa deposit by fetching it from the REST API.
/// Useful for deposits that fell outside the normal lookback window.
///
/// POST /kaspa/deposit-force
/// Body: { "kaspa_tx": "242b5987..." }
pub async fn handler(
    State(state): State<ServerState>,
    Json(body): Json<RequestBody>,
) -> ServerResult<ServerSuccessResponse<ResponseBody>> {
    let RequestBody { kaspa_tx } = body;
    tracing::info!(%kaspa_tx, "Received deposit force request");

    let (sender, client) = match (&state.force_sender, &state.http_client) {
        (Some(s), Some(c)) => (s, c),
        _ => {
            return Err(ServerErrorResponse::new(
                StatusCode::SERVICE_UNAVAILABLE,
                ServerErrorBody {
                    message: "Deposit force is not enabled on this relayer".to_string(),
                },
            ));
        }
    };

    let tx = client.get_tx_by_id(&kaspa_tx).await.map_err(|e| {
        tracing::error!(%kaspa_tx, error = ?e, "Failed to fetch transaction from Kaspa API");
        ServerErrorResponse::new(
            StatusCode::NOT_FOUND,
            ServerErrorBody {
                message: format!("Transaction not found or API error: {}", e),
            },
        )
    })?;

    let deposit: Deposit = tx.try_into().map_err(|e: eyre::Error| {
        tracing::error!(%kaspa_tx, error = ?e, "Failed to convert transaction to deposit");
        ServerErrorResponse::new(
            StatusCode::BAD_REQUEST,
            ServerErrorBody {
                message: format!("Invalid deposit transaction: {}", e),
            },
        )
    })?;

    let deposit_id = deposit.id.to_string();

    sender.send(deposit).await.map_err(|e| {
        tracing::error!(%kaspa_tx, error = ?e, "Failed to send deposit to processing channel");
        ServerErrorResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ServerErrorBody {
                message: "Failed to queue deposit for processing".to_string(),
            },
        )
    })?;

    tracing::info!(%kaspa_tx, %deposit_id, "Deposit queued for processing");

    Ok(ServerSuccessResponse::new(ResponseBody {
        message: "Deposit queued for processing".to_string(),
        deposit_id,
    }))
}
