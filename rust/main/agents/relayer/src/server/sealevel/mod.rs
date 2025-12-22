use std::collections::HashMap;

use axum::{routing::get, Router};
use derive_new::new;
use hyperlane_base::db::HyperlaneRocksDB;
use tower_http::cors::{Any, CorsLayer};

pub mod delivered;

#[derive(Clone, Debug, new)]
pub struct ServerState {
    pub dbs: HashMap<u32, HyperlaneRocksDB>,
}

impl ServerState {
    pub fn router(self) -> Router {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        Router::new()
            .route("/sealevel/delivered", get(delivered::handler))
            .layer(cors)
            .with_state(self)
    }
}

