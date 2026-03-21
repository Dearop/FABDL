/// Account-related XRPL queries: `account_lines` and `account_tx`.
use serde_json::json;

use crate::{
    error::AnalysisError,
    types::xrpl::{AccountLinesResponse, AccountTxResponse},
};

use super::client::HttpXrplClient;

pub(super) async fn fetch_account_lines(
    client: &HttpXrplClient,
    account: &str,
) -> Result<AccountLinesResponse, AnalysisError> {
    let result = client
        .rpc("account_lines", json!({ "account": account, "ledger_index": "validated" }))
        .await?;

    serde_json::from_value(result).map_err(AnalysisError::Json)
}

pub(super) async fn fetch_account_tx(
    client: &HttpXrplClient,
    account: &str,
    limit: u32,
    marker: Option<serde_json::Value>,
) -> Result<AccountTxResponse, AnalysisError> {
    let mut params = json!({
        "account": account,
        "limit": limit,
        "forward": false    // newest first
    });

    if let Some(m) = marker {
        params["marker"] = m;
    }

    let result = client.rpc("account_tx", params).await?;

    serde_json::from_value(result).map_err(AnalysisError::Json)
}
