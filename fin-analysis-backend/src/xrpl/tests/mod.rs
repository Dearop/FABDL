/// Unit tests for the XRPL module using a `MockXrplClient`.
use async_trait::async_trait;
use serde_json::Value;

use crate::{
    error::AnalysisError,
    types::{
        pool::{PoolSnapshot, PricePoint},
        quant::{LendingVaultSnapshot, LoanPosition},
        xrpl::{
            AccountLinesResponse, AccountTxResponse, AmountField, AmmInfo, AmmInfoResponse,
            LpTokenInfo, TrustLine,
        },
    },
    xrpl::XrplClient,
};

// ---------------------------------------------------------------------------
// MockXrplClient
// ---------------------------------------------------------------------------

pub struct MockXrplClient {
    pub xrp_price: f64,
    pub amm_response: Option<AmmInfoResponse>,
    pub account_lines: Vec<TrustLine>,
    pub account_lines_error: Option<String>,
}

impl Default for MockXrplClient {
    fn default() -> Self {
        Self {
            xrp_price: 0.50,
            amm_response: Some(make_amm_response()),
            account_lines: vec![make_lp_line()],
            account_lines_error: None,
        }
    }
}

fn make_amm_response() -> AmmInfoResponse {
    AmmInfoResponse {
        amm: AmmInfo {
            account: "rAMMPoolXRPUSD".to_string(),
            amount: AmountField::Xrp("1000000000".to_string()), // 1000 XRP
            amount2: AmountField::Token {
                value: "500.0".to_string(),
                currency: "USD".to_string(),
                issuer: "rUSDIssuer".to_string(),
            },
            lp_token: LpTokenInfo {
                value: "10000.0".to_string(),
                currency: "03930D02".to_string(),
                issuer: "rAMMPoolXRPUSD".to_string(),
            },
            trading_fee: 30,
            auction_slot: None,
            vote_slots: None,
        },
    }
}

fn make_lp_line() -> TrustLine {
    TrustLine {
        currency: "03930D02".to_string(),
        issuer: "rAMMPoolXRPUSD".to_string(),
        balance: "250.0".to_string(), // 2.5 % share of 10000 LP tokens
        limit: "1000000".to_string(),
        limit_peer: None,
    }
}

#[async_trait]
impl XrplClient for MockXrplClient {
    async fn amm_info(
        &self,
        _asset_currency: &str,
        _asset2_currency: &str,
        _asset2_issuer: Option<&str>,
    ) -> Result<AmmInfoResponse, AnalysisError> {
        self.amm_response
            .clone()
            .ok_or_else(|| AnalysisError::PoolNotFound("XRP/USD".to_string()))
    }

    async fn account_lines(&self, _account: &str) -> Result<AccountLinesResponse, AnalysisError> {
        if let Some(error) = &self.account_lines_error {
            return Err(AnalysisError::XrplRpc(error.clone()));
        }

        Ok(AccountLinesResponse {
            lines: self.account_lines.clone(),
            marker: None,
        })
    }

    async fn account_tx(
        &self,
        _account: &str,
        _limit: u32,
        _marker: Option<Value>,
    ) -> Result<AccountTxResponse, AnalysisError> {
        Ok(AccountTxResponse {
            transactions: Vec::new(),
            marker: None,
        })
    }

    async fn xrp_usd_price(&self) -> Result<f64, AnalysisError> {
        Ok(self.xrp_price)
    }

    async fn price_history(&self, days: u32) -> Result<Vec<PricePoint>, AnalysisError> {
        // Return a flat price history for the requested number of days.
        let points = (0..=days)
            .map(|i| PricePoint {
                timestamp_secs: i as u64 * 86_400,
                xrp_usd: self.xrp_price,
            })
            .collect();
        Ok(points)
    }

    async fn lending_vault_info(
        &self,
        asset: &str,
    ) -> Result<LendingVaultSnapshot, AnalysisError> {
        Ok(LendingVaultSnapshot {
            asset: asset.to_string(),
            ..Default::default()
        })
    }

    async fn account_loans(
        &self,
        _wallet: &str,
    ) -> Result<Vec<LoanPosition>, AnalysisError> {
        Ok(vec![])
    }

    async fn fetch_pool_snapshot(
        &self,
        wallet: &str,
        pool_label: &str,
    ) -> Result<PoolSnapshot, AnalysisError> {
        let amm_resp = self.amm_info("XRP", "USD", None).await?;
        let xrp_price = self.xrp_usd_price().await?;
        let history = self.price_history(90).await?;
        let lines = self.account_lines(wallet).await?;

        let lp_currency = &amm_resp.amm.lp_token.currency;
        let lp_issuer = &amm_resp.amm.lp_token.issuer;
        let lp_supply: f64 = amm_resp.amm.lp_token.value.parse().unwrap_or(0.0);

        let reserve_xrp_drops: u128 = match &amm_resp.amm.amount {
            AmountField::Xrp(s) => s.parse().unwrap_or(0),
            _ => 0,
        };
        let reserve_token_raw: u128 = amm_resp.amm.amount2.parse_raw();

        let positions = lines
            .lines
            .iter()
            .filter(|l| &l.currency == lp_currency && &l.issuer == lp_issuer)
            .map(|l| {
                let lp_held: f64 = l.balance.parse().unwrap_or(0.0);
                let share = if lp_supply > 0.0 { lp_held / lp_supply } else { 0.0 };
                crate::types::pool::PositionSnapshot {
                    owner: wallet.to_string(),
                    lower_tick: 0,
                    upper_tick: 0,
                    liquidity: 0,
                    fee_growth_inside_0_last_q128: 0,
                    fee_growth_inside_1_last_q128: 0,
                    amount0_at_entry: (reserve_xrp_drops as f64 / 1_000_000.0) * share,
                    amount1_at_entry: (reserve_token_raw as f64 / 1_000_000.0) * share,
                    entry_price_usd: xrp_price,
                    lp_tokens_held: lp_held,
                }
            })
            .collect();

        Ok(PoolSnapshot {
            pool_label: pool_label.to_string(),
            amm_account: amm_resp.amm.account,
            reserve_xrp_drops,
            reserve_token_raw,
            token_currency: "USD".to_string(),
            token_issuer: "rUSDIssuer".to_string(),
            lp_token_supply: lp_supply,
            trading_fee_bps: amm_resp.amm.trading_fee,
            current_xrp_price_usd: xrp_price,
            sqrt_price_q64: None,
            current_tick: None,
            liquidity_active: None,
            fee_growth_global_0_q128: None,
            fee_growth_global_1_q128: None,
            ticks: Vec::new(),
            positions,
            price_history: history,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mock_amm_info_returns_pool() {
    let client = MockXrplClient::default();
    let resp = client.amm_info("XRP", "USD", None).await.unwrap();
    assert_eq!(resp.amm.account, "rAMMPoolXRPUSD");
    assert_eq!(resp.amm.trading_fee, 30);
}

#[tokio::test]
async fn drops_to_xrp_conversion() {
    let client = MockXrplClient::default();
    let resp = client.amm_info("XRP", "USD", None).await.unwrap();
    let drops = match &resp.amm.amount {
        crate::types::xrpl::AmountField::Xrp(s) => s.parse::<u128>().unwrap(),
        _ => panic!("expected XRP"),
    };
    // 1_000_000_000 drops = 1000 XRP
    assert_eq!(drops / 1_000_000, 1_000u128);
}

#[tokio::test]
async fn pool_not_found_propagates() {
    let client = MockXrplClient {
        amm_response: None,
        ..Default::default()
    };
    let result = client.amm_info("XRP", "USD", None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn lp_token_filter_finds_position() {
    let client = MockXrplClient::default();
    let snapshot = client.fetch_pool_snapshot("rWallet", "XRP/USD").await.unwrap();
    assert_eq!(snapshot.positions.len(), 1);
    // The position holds 250 LP tokens out of 10000 total = 2.5 % share.
    let pos = &snapshot.positions[0];
    assert!((pos.lp_tokens_held - 250.0).abs() < 1e-6);
}

#[tokio::test]
async fn pool_snapshot_has_correct_reserves() {
    let client = MockXrplClient::default();
    let snapshot = client.fetch_pool_snapshot("rWallet", "XRP/USD").await.unwrap();
    // 1_000_000_000 drops = 1000 XRP
    assert_eq!(snapshot.reserve_xrp_drops, 1_000_000_000);
    assert!((snapshot.reserve_xrp() - 1000.0).abs() < 1e-4);
}
