use std::sync::Arc;

use axum::{routing::get, Router};
use derive_new::new;
use hyperlane_base::kas_hack::KaspaRocksDB;

pub mod list_deposits;
pub mod list_withdrawals;

#[derive(Clone, Debug, new)]
pub struct ServerState {
    pub kaspa_db: Arc<KaspaRocksDB>,
}

impl ServerState {
    pub fn router(self) -> Router {
        Router::new()
            .route("/kaspa/deposit", get(list_deposits::handler))
            .route("/kaspa/withdrawal", get(list_withdrawals::handler))
            .with_state(self)
    }
}
