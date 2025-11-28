pub mod eigen_node;
pub mod merkle_tree_insertions;

pub use eigen_node::EigenNodeApi;

use std::sync::Arc;

use axum::{routing::get, Json, Router};
use serde::{Deserialize, Serialize};

use hyperlane_base::CoreMetrics;
use hyperlane_core::HyperlaneDomain;

use crate::ValidatorMetadata;

#[derive(Serialize, Deserialize, Debug)]
struct VersionResponse {
    git_sha: String,
}

/// Returns a vector of validator-specific endpoint routes to be served.
/// Can be extended with additional routes and feature flags to enable/disable individually.
pub fn router(
    origin_chain: HyperlaneDomain,
    metrics: Arc<CoreMetrics>,
    metadata: Arc<ValidatorMetadata>,
) -> Router {
    let eigen_node_api = EigenNodeApi::new(origin_chain, metrics);

    let metadata_clone = metadata.clone();
    let version_handler = get(move || async move {
        let response = VersionResponse {
            git_sha: metadata_clone.git_sha.clone(),
        };
        Json(response)
    });

    eigen_node_api.router().route("/version", version_handler)
}
