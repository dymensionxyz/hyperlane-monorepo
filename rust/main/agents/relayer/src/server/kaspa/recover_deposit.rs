use axum::{extract::State, http::StatusCode, Json};
use dymension_kaspa::dym_kas_core::api::client::Deposit;
use serde::{Deserialize, Serialize};

use hyperlane_base::server::utils::{
    ServerErrorBody, ServerErrorResponse, ServerResult, ServerSuccessResponse,
};

use super::ServerState;

#[derive(Clone, Debug, Deserialize)]
pub struct RequestBody {
    /// The Kaspa transaction ID to recover
    pub kaspa_tx: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResponseBody {
    pub message: String,
    pub deposit_id: String,
}

/// Recover a Kaspa deposit by fetching it from the Kaspa REST API and submitting
/// it for processing. This is useful for deposits that fell outside the normal
/// lookback window due to relayer DB being wiped or other issues.
///
/// POST /kaspa/deposit/recover
/// Body: { "kaspa_tx": "242b5987..." }
pub async fn handler(
    State(state): State<ServerState>,
    Json(body): Json<RequestBody>,
) -> ServerResult<ServerSuccessResponse<ResponseBody>> {
    let RequestBody { kaspa_tx } = body;
    tracing::info!(%kaspa_tx, "Received deposit recovery request");

    // Check if recovery is enabled
    let recovery = state.recovery.as_ref().ok_or_else(|| {
        ServerErrorResponse::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ServerErrorBody {
                message: "Deposit recovery is not enabled on this relayer".to_string(),
            },
        )
    })?;

    // Fetch the transaction from Kaspa REST API
    let tx = recovery
        .http_client
        .get_tx_by_id(&kaspa_tx)
        .await
        .map_err(|e| {
            tracing::error!(%kaspa_tx, error = ?e, "Failed to fetch transaction from Kaspa API");
            ServerErrorResponse::new(
                StatusCode::NOT_FOUND,
                ServerErrorBody {
                    message: format!("Transaction not found or API error: {}", e),
                },
            )
        })?;

    // Validate it's a valid escrow transfer
    if !is_valid_escrow_transfer(&tx, &recovery.escrow_address) {
        return Err(ServerErrorResponse::new(
            StatusCode::BAD_REQUEST,
            ServerErrorBody {
                message: format!(
                    "Transaction {} is not a valid deposit to escrow {}",
                    kaspa_tx, recovery.escrow_address
                ),
            },
        ));
    }

    // Convert to Deposit
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

    // Send to recovery channel
    recovery.sender.send(deposit).await.map_err(|e| {
        tracing::error!(%kaspa_tx, error = ?e, "Failed to send deposit to recovery channel");
        ServerErrorResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ServerErrorBody {
                message: "Failed to queue deposit for recovery".to_string(),
            },
        )
    })?;

    tracing::info!(%kaspa_tx, %deposit_id, "Deposit queued for recovery");

    Ok(ServerSuccessResponse::new(ResponseBody {
        message: "Deposit queued for recovery processing".to_string(),
        deposit_id,
    }))
}

fn is_valid_escrow_transfer(
    tx: &dymension_kaspa::dym_kas_api::models::TxModel,
    escrow_address: &str,
) -> bool {
    tx.outputs.as_ref().map_or(false, |outputs| {
        outputs.iter().any(|utxo| {
            utxo.script_public_key_address
                .as_ref()
                .map_or(false, |dest| dest == escrow_address)
        })
    })
}
