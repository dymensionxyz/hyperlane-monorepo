use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use dymension_kaspa::dym_kas_core::api::{base::RateLimitConfig, client::HttpClient};
use hyperlane_base::kas_hack::DepositRecoverySender;
use hyperlane_core::KaspaDb;
use tower_http::cors::{Any, CorsLayer};

pub mod list_deposits;
pub mod list_withdrawals;
pub mod recover_deposit;

/// Configuration for deposit recovery functionality
#[derive(Clone)]
pub struct RecoveryConfig {
    pub sender: DepositRecoverySender,
    pub http_client: HttpClient,
    pub escrow_address: String,
}

/// Server state for Kaspa endpoints
#[derive(Clone)]
pub struct ServerState {
    pub kaspa_db: Arc<dyn KaspaDb>,
    /// Optional recovery configuration (sender, HTTP client, escrow address)
    pub recovery: Option<RecoveryConfig>,
}

impl std::fmt::Debug for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerState")
            .field("kaspa_db", &"<dyn KaspaDb>")
            .field("recovery", &self.recovery.is_some())
            .finish()
    }
}

impl ServerState {
    pub fn new(kaspa_db: Arc<dyn KaspaDb>) -> Self {
        Self {
            kaspa_db,
            recovery: None,
        }
    }

    pub fn with_recovery(
        mut self,
        sender: DepositRecoverySender,
        rest_api_url: String,
        escrow_address: String,
    ) -> Self {
        let http_client = HttpClient::new(rest_api_url, RateLimitConfig::default());
        self.recovery = Some(RecoveryConfig {
            sender,
            http_client,
            escrow_address,
        });
        self
    }

    pub fn router(self) -> Router {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        Router::new()
            .route("/kaspa/deposit", get(list_deposits::handler))
            .route("/kaspa/withdrawal", get(list_withdrawals::handler))
            .route("/kaspa/deposit/recover", post(recover_deposit::handler))
            .layer(cors)
            .with_state(self)
    }
}
