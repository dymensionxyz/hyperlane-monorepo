use std::collections::HashMap;
use std::sync::Arc;

use axum::{routing::get, Router};
use hyperlane_base::{ContractSyncer, db::HyperlaneRocksDB};
use hyperlane_core::HyperlaneMessage;
use tower_http::cors::{Any, CorsLayer};

pub mod dispatched;
pub mod handler;

// Import ScraperDb - this is from the scraper crate
// We'll need to make sure scraper is a dependency of relayer
use crate::scraper_db::ScraperDb;

#[derive(Clone)]
pub struct ServerState {
    pub dbs: HashMap<u32, HyperlaneRocksDB>,
    /// Message syncs for chain queries by the /delivered endpoint (domain_id -> message_sync)
    pub message_syncs: HashMap<u32, Arc<dyn ContractSyncer<HyperlaneMessage>>>,
    /// Optional scraper database for fallback delivery lookups
    pub scraper_db: Option<ScraperDb>,
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
        Self { 
            dbs, 
            message_syncs,
            scraper_db: None,
        }
    }

    pub fn with_scraper_db(mut self, scraper_db: Option<ScraperDb>) -> Self {
        self.scraper_db = scraper_db;
        self
    }
}

impl ServerState {
    pub fn router(self) -> Router {
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

