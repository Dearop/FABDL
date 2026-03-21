/// HTTP server wiring (axum).
///
/// Exposes a single endpoint:
///   POST /analyze  →  accepts `IntentRouterOutput` JSON, returns `PortfolioRiskSummary` JSON.
pub mod handlers;

use std::sync::Arc;

use axum::{routing::post, Router};

use crate::pipeline::AnalysisPipeline;

/// Build the axum `Router` with the analysis pipeline injected.
pub fn build_router(pipeline: Arc<dyn AnalysisPipeline>) -> Router {
    Router::new().route("/analyze", post(handlers::analyze_handler)).with_state(pipeline)
}

#[cfg(test)]
mod tests;
