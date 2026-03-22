use std::{net::SocketAddr, sync::Arc};

use fin_analysis_backend::{
    pipeline::DefaultPipeline,
    server::build_router,
    xrpl::HttpXrplClient,
};

#[tokio::main]
async fn main() {
    // Initialise structured logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "fin_analysis_backend=info".into()),
        )
        .init();

    // Configuration from environment (with sensible defaults for local dev).
    let xrpl_endpoint = std::env::var("XRPL_ENDPOINT")
        .unwrap_or_else(|_| "https://lend.devnet.rippletest.net:51234".to_string());

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3001);

    let network = if xrpl_endpoint.contains("lend.devnet") {
        "lending-devnet"
    } else if xrpl_endpoint.contains("altnet") || xrpl_endpoint.contains("rippletest") {
        "xrpl-testnet-like"
    } else {
        "custom"
    };

    tracing::info!(endpoint = %xrpl_endpoint, network, port, "starting fin-analysis-backend");

    let xrpl_client = HttpXrplClient::new(xrpl_endpoint);
    let pipeline = Arc::new(DefaultPipeline::new(xrpl_client));
    let app = build_router(pipeline);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!("listening on {addr}");
    axum::serve(listener, app).await.unwrap();
}
