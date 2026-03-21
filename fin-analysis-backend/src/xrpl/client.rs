/// `HttpXrplClient` — production XRPL JSON-RPC client.
///
/// Sends JSON-RPC 2.0 requests to the configured XRPL node endpoint using
/// `reqwest`.  All XRPL-specific request building and response parsing is
/// delegated to the sibling modules `amm`, `account`, and `price_feed`.
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use crate::{
    error::AnalysisError,
    types::{
        pool::{PoolSnapshot, PricePoint},
        xrpl::{AccountLinesResponse, AccountTxResponse, AmmInfoResponse},
    },
};

use super::{account, amm, price_feed, XrplClient};

pub struct HttpXrplClient {
    http: Client,
    /// XRPL JSON-RPC endpoint, e.g. "https://xrplcluster.com".
    pub endpoint: String,
    /// Optional price feed URL override (defaults to CoinGecko).
    pub price_feed_url: Option<String>,
}

impl HttpXrplClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            endpoint: endpoint.into(),
            price_feed_url: None,
        }
    }

    /// POST a JSON-RPC request and return the `result` field.
    pub(crate) async fn rpc(&self, method: &str, params: Value) -> Result<Value, AnalysisError> {
        let body = json!({
            "method": method,
            "params": [params]
        });

        let resp = self
            .http
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await?
            .json::<Value>()
            .await?;

        // XRPL returns errors in `result.error` or top-level `error`.
        if let Some(err) = resp.get("error") {
            return Err(AnalysisError::XrplRpc(err.to_string()));
        }
        let result = resp
            .get("result")
            .ok_or_else(|| AnalysisError::XrplRpc("missing 'result' field".into()))?
            .clone();

        if let Some(status) = result.get("status") {
            if status != "success" {
                let err_msg = result
                    .get("error_message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                return Err(AnalysisError::XrplRpc(err_msg.to_string()));
            }
        }

        Ok(result)
    }
}

#[async_trait]
impl XrplClient for HttpXrplClient {
    async fn amm_info(
        &self,
        asset_currency: &str,
        asset2_currency: &str,
        asset2_issuer: Option<&str>,
    ) -> Result<AmmInfoResponse, AnalysisError> {
        amm::fetch_amm_info(self, asset_currency, asset2_currency, asset2_issuer).await
    }

    async fn account_lines(
        &self,
        account_addr: &str,
    ) -> Result<AccountLinesResponse, AnalysisError> {
        account::fetch_account_lines(self, account_addr).await
    }

    async fn account_tx(
        &self,
        account_addr: &str,
        limit: u32,
        marker: Option<serde_json::Value>,
    ) -> Result<AccountTxResponse, AnalysisError> {
        account::fetch_account_tx(self, account_addr, limit, marker).await
    }

    async fn xrp_usd_price(&self) -> Result<f64, AnalysisError> {
        price_feed::fetch_xrp_usd_price(&self.http, self.price_feed_url.as_deref()).await
    }

    async fn price_history(&self, days: u32) -> Result<Vec<PricePoint>, AnalysisError> {
        price_feed::fetch_price_history(&self.http, days, self.price_feed_url.as_deref()).await
    }

    async fn fetch_pool_snapshot(
        &self,
        wallet: &str,
        pool_label: &str,
    ) -> Result<PoolSnapshot, AnalysisError> {
        amm::build_pool_snapshot(self, wallet, pool_label).await
    }
}
