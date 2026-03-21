/// Unified error type for the analysis backend.
#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("XRPL RPC error: {0}")]
    XrplRpc(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Math overflow in fixed-point operation")]
    MathOverflow,

    #[error("Missing required parameter: {0}")]
    MissingParameter(&'static str),

    #[error("Pool not found: {0}")]
    PoolNotFound(String),

    #[error("Insufficient price history: need {need} points, got {got}")]
    InsufficientHistory { need: usize, got: usize },

    #[error("Division by zero")]
    DivisionByZero,
}
