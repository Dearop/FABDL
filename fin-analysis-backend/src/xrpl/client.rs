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
        quant::{LendingVaultSnapshot, LoanPosition},
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

    async fn lending_vault_info(
        &self,
        asset: &str,
    ) -> Result<LendingVaultSnapshot, AnalysisError> {
        let params = json!({
            "ledger_entry": {
                "LendingPool": {
                    "asset": { "currency": asset }
                }
            }
        });

        let result = match self.rpc("ledger_entry", params).await {
            Ok(r) => r,
            Err(_) => {
                return Ok(LendingVaultSnapshot {
                    asset: asset.to_string(),
                    ..Default::default()
                });
            }
        };

        let node = match result.get("node") {
            Some(n) => n,
            None => {
                return Ok(LendingVaultSnapshot {
                    asset: asset.to_string(),
                    ..Default::default()
                });
            }
        };

        let total_supply: f64 = node
            .get("TotalSupply")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .map(|drops: f64| drops / 1_000_000.0)
            .unwrap_or(0.0);

        let total_borrow: f64 = node
            .get("TotalBorrow")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .map(|drops: f64| drops / 1_000_000.0)
            .unwrap_or(0.0);

        let supply_apy: f64 = node
            .get("SupplyAPY")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);

        let borrow_apy: f64 = node
            .get("BorrowAPY")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);

        let utilization_rate = if total_supply > 0.0 {
            total_borrow / total_supply
        } else {
            0.0
        };

        Ok(LendingVaultSnapshot {
            asset: asset.to_string(),
            total_supply_usd: total_supply,
            total_borrow_usd: total_borrow,
            utilization_rate,
            supply_apy,
            borrow_apy,
        })
    }

    async fn account_loans(
        &self,
        wallet: &str,
    ) -> Result<Vec<LoanPosition>, AnalysisError> {
        let params = json!({
            "account": wallet,
            "type": "loan"
        });

        let result = match self.rpc("account_objects", params).await {
            Ok(r) => r,
            Err(_) => return Ok(vec![]),
        };

        let objects = match result.get("account_objects").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => return Ok(vec![]),
        };

        let loans = objects
            .iter()
            .filter_map(|obj| {
                let asset_borrowed = obj
                    .get("BorrowedAsset")
                    .and_then(|v| v.get("currency"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let amount_borrowed_usd: f64 = obj
                    .get("BorrowedAmount")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
                    .map(|drops: f64| drops / 1_000_000.0)
                    .unwrap_or(0.0);

                let collateral_asset = obj
                    .get("CollateralAsset")
                    .and_then(|v| v.get("currency"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let collateral_usd: f64 = obj
                    .get("CollateralAmount")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
                    .map(|drops: f64| drops / 1_000_000.0)
                    .unwrap_or(0.0);

                let health_factor: f64 = obj
                    .get("HealthFactor")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);

                let borrow_apy: f64 = obj
                    .get("BorrowAPY")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);

                let term_days: Option<u32> = obj
                    .get("TermDays")
                    .and_then(|v| v.as_u64())
                    .map(|d| d as u32);

                Some(LoanPosition {
                    asset_borrowed,
                    amount_borrowed_usd,
                    collateral_asset,
                    collateral_usd,
                    health_factor,
                    borrow_apy,
                    term_days,
                })
            })
            .collect();

        Ok(loans)
    }
}
