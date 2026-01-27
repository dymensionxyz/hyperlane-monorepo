use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use dymension_kaspa::dym_kas_core::api::{base::RateLimitConfig, client::HttpClient};
use hyperlane_base::kas_hack::DepositForceSender;
use hyperlane_core::KaspaDb;
use tower_http::cors::{Any, CorsLayer};

pub mod deposit_force;
pub mod list_deposits;
pub mod list_withdrawals;

#[derive(Clone)]
pub struct ServerState {
    pub kaspa_db: Arc<dyn KaspaDb>,
    pub force_sender: Option<DepositForceSender>,
    pub http_client: Option<HttpClient>,
}

impl std::fmt::Debug for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerState")
            .field("kaspa_db", &"<dyn KaspaDb>")
            .field("force_sender", &self.force_sender.is_some())
            .field("http_client", &self.http_client.is_some())
            .finish()
    }
}

impl ServerState {
    pub fn new(kaspa_db: Arc<dyn KaspaDb>) -> Self {
        Self {
            kaspa_db,
            force_sender: None,
            http_client: None,
        }
    }

    pub fn with_deposit_force(mut self, sender: DepositForceSender, rest_api_url: String) -> Self {
        self.force_sender = Some(sender);
        self.http_client = Some(HttpClient::new(rest_api_url, RateLimitConfig::default()));
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
            .route("/kaspa/deposit-force", post(deposit_force::handler))
            .layer(cors)
            .with_state(self)
    }
}
