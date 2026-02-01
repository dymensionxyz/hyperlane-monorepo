use std::collections::HashMap;

use axum::{routing::get, Router};
use hyperlane_base::db::HyperlaneRocksDB;
use tower_http::cors::{Any, CorsLayer};

pub mod dispatched;
pub mod handler;

#[derive(Clone, Debug)]
pub struct ServerState {
    pub dbs: HashMap<u32, HyperlaneRocksDB>,
}

impl ServerState {
    pub fn new(dbs: HashMap<u32, HyperlaneRocksDB>) -> Self {
        Self { dbs }
    }

    pub fn router(self) -> Router {
        // Note: CORS is permissive (Any origin/method/header) as these are public read-only APIs
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        Router::new()
            .route("/delivered", get(handler::handler))
            .route("/dispatched", get(dispatched::handler))
            .layer(cors)
            .with_state(self)
    }
}
