/// XRPL data fetching layer.
///
/// The `XrplClient` trait abstracts all calls to the XRPL JSON-RPC endpoint.
/// `HttpXrplClient` is the production implementation; tests use
/// `MockXrplClient` defined in the `tests` sub-module.
pub mod account;
pub mod amm;
pub mod client;
pub mod price_feed;

pub use client::HttpXrplClient;

use async_trait::async_trait;

use crate::{
    error::AnalysisError,
    types::{
        pool::{PoolSnapshot, PricePoint},
        quant::{LendingVaultSnapshot, LoanPosition},
        xrpl::{AccountLinesResponse, AccountTxResponse, AmmInfoResponse},
    },
};

// ---------------------------------------------------------------------------
// XrplClient trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait XrplClient: Send + Sync {
    /// Fetch AMM pool state for a given asset pair.
    async fn amm_info(
        &self,
        asset_currency: &str,
        asset2_currency: &str,
        asset2_issuer: Option<&str>,
    ) -> Result<AmmInfoResponse, AnalysisError>;

    /// Fetch trust-line balances (including LP tokens) for an account.
    async fn account_lines(
        &self,
        account: &str,
    ) -> Result<AccountLinesResponse, AnalysisError>;

    /// Fetch recent transactions for an account.
    async fn account_tx(
        &self,
        account: &str,
        limit: u32,
        marker: Option<serde_json::Value>,
    ) -> Result<AccountTxResponse, AnalysisError>;

    /// Fetch current XRP/USD spot price.
    async fn xrp_usd_price(&self) -> Result<f64, AnalysisError>;

    /// Fetch historical XRP/USD prices for the past `days` days.
    async fn price_history(&self, days: u32) -> Result<Vec<PricePoint>, AnalysisError>;

    /// High-level helper: build a normalised `PoolSnapshot` for one pool.
    async fn fetch_pool_snapshot(
        &self,
        wallet: &str,
        pool_label: &str,
    ) -> Result<PoolSnapshot, AnalysisError>;

    /// Fetch XLS-66d lending vault info for a given asset.
    async fn lending_vault_info(
        &self,
        asset: &str,
    ) -> Result<LendingVaultSnapshot, AnalysisError>;

    /// Fetch XLS-66d open loan positions for a wallet.
    async fn account_loans(
        &self,
        wallet: &str,
    ) -> Result<Vec<LoanPosition>, AnalysisError>;
}

#[cfg(test)]
pub(crate) mod tests;
