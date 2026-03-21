/// Axum request handlers.
use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use axum::http::header;
use serde_json::json;

use crate::{
    error::AnalysisError,
    pipeline::AnalysisPipeline,
    types::intent::IntentRouterOutput,
};

/// `POST /analyze`
///
/// Accepts a JSON `IntentRouterOutput`, runs the analysis pipeline, and
/// returns a JSON `PortfolioRiskSummary`.
pub async fn analyze_handler(
    State(pipeline): State<Arc<dyn AnalysisPipeline>>,
    Json(intent): Json<IntentRouterOutput>,
) -> Response {
    match pipeline.run(intent).await {
        Ok(summary) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            summary.render_prompt(),
        )
            .into_response(),
        Err(e) => error_response(e),
    }
}

fn error_response(err: AnalysisError) -> Response {
    let (status, msg) = match &err {
        AnalysisError::MissingParameter(_) => (StatusCode::BAD_REQUEST, err.to_string()),
        AnalysisError::PoolNotFound(_) => (StatusCode::NOT_FOUND, err.to_string()),
        AnalysisError::XrplRpc(_) => (StatusCode::BAD_GATEWAY, err.to_string()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    };

    (status, Json(json!({ "error": msg }))).into_response()
}
