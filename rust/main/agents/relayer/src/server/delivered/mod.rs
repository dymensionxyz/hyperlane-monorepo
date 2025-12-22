use std::collections::HashMap;
use std::sync::Arc;

use axum::{routing::get, Router};
use hyperlane_base::{ContractSyncer, db::HyperlaneRocksDB};
use hyperlane_core::HyperlaneMessage;
use tower_http::cors::{Any, CorsLayer};
use tracing::warn;

pub mod by_tx;
pub mod handler;

#[derive(Clone)]
pub struct ServerState {
    pub dbs: HashMap<u32, HyperlaneRocksDB>,
    /// Message syncs for chain queries (domain_id -> message_sync)
    pub message_syncs: HashMap<u32, Arc<dyn ContractSyncer<HyperlaneMessage>>>,
}

impl std::fmt::Debug for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerState")
            .field("dbs", &self.dbs)
            .field("message_syncs", &format!("HashMap<u32, Arc<dyn ContractSyncer>> ({} entries)", self.message_syncs.len()))
            .finish()
    }
}

impl ServerState {
    pub fn new(
        dbs: HashMap<u32, HyperlaneRocksDB>,
        message_syncs: HashMap<u32, Arc<dyn ContractSyncer<HyperlaneMessage>>>,
    ) -> Self {
        Self { dbs, message_syncs }
    }
}

impl ServerState {
    pub fn router(self) -> Router {
        let dbs_count = self.dbs.len();
        let domain_ids: Vec<u32> = self.dbs.keys().copied().collect();
        
        warn!(
            dbs_count = %dbs_count,
            domain_ids = ?domain_ids,
            "DELIVERY_API: Registering /delivered endpoint"
        );

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        Router::new()
            .route("/delivered", get(handler::handler))
            .route("/delivered/by_tx", get(by_tx::handler))
            .layer(cors)
            .with_state(self)
    }
}

