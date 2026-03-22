/// Integration tests for the HTTP server using `axum-test`.
use std::sync::Arc;

use axum_test::TestServer;
use serde_json::json;

use crate::{
    pipeline::DefaultPipeline,
    server::build_router,
    xrpl::tests::MockXrplClient,
};

fn make_server() -> TestServer {
    let pipeline = Arc::new(DefaultPipeline::new(MockXrplClient::default()));
    let app = build_router(pipeline);
    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn post_analyze_get_price_returns_200() {
    let server = make_server();
    let body = json!({
        "action": "get_price",
        "scope": "portfolio",
        "parameters": {}
    });
    let resp = server.post("/analyze").json(&body).await;
    resp.assert_status_ok();
    let text = resp.text();
    assert!(text.contains("Current XRP Price"), "unexpected response body: {text}");
}

#[tokio::test]
async fn post_analyze_bad_json_returns_422() {
    let server = make_server();
    let resp = server
        .post("/analyze")
        .bytes(b"not json at all".as_ref().into())
        .content_type("application/json")
        .await;
    // axum returns 422 for malformed JSON body
    assert!(
        resp.status_code().is_client_error(),
        "malformed JSON should return 4xx"
    );
}

#[tokio::test]
async fn post_analyze_missing_wallet_returns_400() {
    let server = make_server();
    let body = json!({
        "action": "analyze_risk",
        "scope": "portfolio",
        "parameters": {}
    });
    let resp = server.post("/analyze").json(&body).await;
    assert_eq!(resp.status_code(), 400);
    let json: serde_json::Value = resp.json();
    assert!(json.get("error").is_some());
}
