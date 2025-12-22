use std::collections::HashMap;

use axum::{routing::get, Router};
use derive_new::new;
use hyperlane_base::db::HyperlaneRocksDB;
use tower_http::cors::{Any, CorsLayer};
use tracing::warn;

pub mod by_tx;
pub mod handler;

#[derive(Clone, Debug, new)]
pub struct ServerState {
    pub dbs: HashMap<u32, HyperlaneRocksDB>,
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

