/// Output types consumed by the Quant LLM (Claude Sonnet) as its context
/// window input.  All fields are plain f64 so the JSON is easy to embed in a
/// prompt without further transformation.
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Per-position risk
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionRisk {
    pub pool_label: String,
    /// Current market value of the position in USD.
    pub position_value_usd: f64,
    /// Impermanent loss as a percentage (negative = loss).
    pub il_pct: f64,
    /// Impermanent loss in USD (negative = loss).
    pub il_usd: f64,
    /// Annualised fee income as a fraction (e.g. 0.15 = 15 % APR).
    pub fee_apr: f64,
    /// Fees earned over the last 7 days in USD.
    pub fees_earned_7d_usd: f64,
    /// Lowest price at which cumulative fees offset IL.
    pub break_even_lower: f64,
    /// Highest price at which cumulative fees offset IL.
    pub break_even_upper: f64,
    /// Net XRP delta exposure (positive = long XRP).
    pub delta_xrp: f64,
    /// Sharpe ratio for this position's price history.
    pub sharpe: f64,
    /// 95 % 1-day Value at Risk in USD (positive number = max expected loss).
    pub var_95_usd: f64,
    /// This position's share of the pool's total LP supply (0–1).
    pub lp_share_pct: f64,
    /// Second-order price sensitivity (gamma) in USD for constant-product AMM.
    pub gamma_usd: f64,
}

// ---------------------------------------------------------------------------
// XLS-66d Lending
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LendingVaultSnapshot {
    pub asset: String,
    pub total_supply_usd: f64,
    pub total_borrow_usd: f64,
    pub utilization_rate: f64,
    pub kink_utilization: f64,
    pub available_liquidity_usd: f64,
    pub supply_apy: f64,
    pub borrow_apy: f64,
}

impl Default for LendingVaultSnapshot {
    fn default() -> Self {
        Self {
            asset: String::new(),
            total_supply_usd: 0.0,
            total_borrow_usd: 0.0,
            utilization_rate: 0.0,
            kink_utilization: 0.0,
            available_liquidity_usd: 0.0,
            supply_apy: 0.0,
            borrow_apy: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanPosition {
    pub asset_borrowed: String,
    pub amount_borrowed_usd: f64,
    pub collateral_asset: String,
    pub collateral_usd: f64,
    pub health_factor: f64,
    pub liquidation_price: f64,
    pub liquidation_penalty_pct: f64,
    pub borrow_apy: f64,
    pub term_days: Option<u32>,
}

// ---------------------------------------------------------------------------
// Portfolio-level risk summary (Quant LLM input)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioRiskSummary {
    /// Total current value of all LP positions in USD.
    pub total_value_usd: f64,
    /// Portfolio-wide IL as a percentage (negative = loss).
    pub impermanent_loss_pct: f64,
    /// Portfolio-wide IL in USD.
    pub impermanent_loss_usd: f64,
    /// Net XRP delta exposure across all positions.
    pub delta_exposure_xrp: f64,
    /// Change in portfolio value (USD) if XRP price drops 10 %.
    pub delta_exposure_usd_if_down_10: f64,
    /// Total fees earned in the last 7 days across all positions (USD).
    pub fee_income_7d: f64,
    /// Current XRP/USD spot price.
    pub current_xrp_price: f64,
    /// Portfolio-level Sharpe ratio (uses the shared price history).
    pub sharpe_ratio: f64,
    /// Portfolio-level 95 % 1-day VaR in USD.
    pub var_95_usd: f64,
    /// Lowest price at which portfolio fees offset IL (weighted average).
    pub break_even_lower: f64,
    /// Highest price at which portfolio fees offset IL (weighted average).
    pub break_even_upper: f64,
    /// Portfolio-wide weighted average fee APR.
    pub fee_apr: f64,
    /// Per-position breakdown.
    pub positions: Vec<PositionRisk>,
    /// XLS-66d lending vault snapshots for assets in the user's pools.
    pub lending_vaults: Vec<LendingVaultSnapshot>,
    /// XLS-66d open loan positions for the user's wallet.
    pub open_loans: Vec<LoanPosition>,
    /// 95 % 1-day Conditional VaR (expected shortfall) in USD.
    pub cvar_95_usd: f64,
    /// Portfolio-level gamma (second-order price sensitivity) in USD.
    pub gamma_usd: f64,
    /// Net carry: fee_apr - weighted_borrow_apy - |IL_pct|/100.
    pub net_carry: f64,
}

impl PortfolioRiskSummary {
    /// Render the summary into a Claude Sonnet prompt string.
    pub fn render_prompt(&self) -> String {
        let mut prompt = format!(
            "Portfolio Risk Summary:\n\
             - Total Value: ${total_value:.0} USD\n\
             - Impermanent Loss: {il_pct:.1}% (-${il_usd:.0})\n\
             - Delta Exposure: {delta_xrp:.0} XRP (~${delta_usd:.0} if XRP drops 10%)\n\
             - 7-Day Fee Income: ${fee_7d:.0}\n\
             - Current XRP Price: ${xrp_price:.4} USD\n\
             - Fee APR: {fee_apr:.1}%\n\
             - Sharpe Ratio: {sharpe:.2}\n\
             - VaR (95%, 1-day): ${var95:.0}\n\
             - CVaR (95%, 1-day expected shortfall): ${cvar95:.0}\n\
             - Gamma: ${gamma:.2}\n\
             - Net Carry: {net_carry:.2}%\n\
             - Break-even Range: ${be_lower:.4} - ${be_upper:.4}\n",
            total_value = self.total_value_usd,
            il_pct = self.impermanent_loss_pct,
            il_usd = self.impermanent_loss_usd.abs(),
            delta_xrp = self.delta_exposure_xrp,
            delta_usd = self.delta_exposure_usd_if_down_10.abs(),
            fee_7d = self.fee_income_7d,
            xrp_price = self.current_xrp_price,
            fee_apr = self.fee_apr * 100.0,
            sharpe = self.sharpe_ratio,
            var95 = self.var_95_usd,
            cvar95 = self.cvar_95_usd,
            gamma = self.gamma_usd,
            net_carry = self.net_carry * 100.0,
            be_lower = self.break_even_lower,
            be_upper = self.break_even_upper,
        );

        let n = self.positions.len();
        if n > 0 {
            let heading = if n == 1 { "AMM Pool Details:" } else { "AMM Pool Details (all positions):" };
            prompt.push_str(&format!("\n{}\n", heading));
            for pos in &self.positions {
                prompt.push_str(&format!(
                    "- Pool: {label}, Value: ${value:.0}, LP Share: {share:.1}%, \
                     IL: {il_pct:.1}% (-${il_usd:.0}), Fee APR: {fee_apr:.1}%, \
                     7d Fees: ${fees_7d:.0}, Delta: {delta:.0} XRP\n",
                    label = pos.pool_label,
                    value = pos.position_value_usd,
                    share = pos.lp_share_pct * 100.0,
                    il_pct = pos.il_pct,
                    il_usd = pos.il_usd.abs(),
                    fee_apr = pos.fee_apr * 100.0,
                    fees_7d = pos.fees_earned_7d_usd,
                    delta = pos.delta_xrp,
                ));
            }
        }

        // Lending context (XLS-66d)
        if !self.lending_vaults.is_empty() || !self.open_loans.is_empty() {
            prompt.push_str("\nLending Context:\n");
            for v in &self.lending_vaults {
                prompt.push_str(&format!(
                    "- Vault {asset}: supply APY {supply:.1}%, borrow APY {borrow:.1}%, utilization {util:.0}%\n",
                    asset = v.asset,
                    supply = v.supply_apy * 100.0,
                    borrow = v.borrow_apy * 100.0,
                    util = v.utilization_rate * 100.0,
                ));
            }
            if !self.open_loans.is_empty() {
                prompt.push_str("Open Loans:\n");
                for loan in &self.open_loans {
                    prompt.push_str(&format!(
                        "- Borrowed {amt:.0} {asset} against {col:.0} {col_asset} collateral, \
                         health factor {hf:.1}, borrow APY {apy:.1}%\n",
                        amt = loan.amount_borrowed_usd,
                        asset = loan.asset_borrowed,
                        col = loan.collateral_usd,
                        col_asset = loan.collateral_asset,
                        hf = loan.health_factor,
                        apy = loan.borrow_apy * 100.0,
                    ));
                }
            }
        }

        let task = if n == 1 {
            "\nTask: Generate 3 strategies to manage this position."
        } else {
            "\nTask: Generate 3 strategies to manage this portfolio."
        };
        prompt.push_str(task);
        prompt
    }

    /// Construct an empty summary (used as a fallback / placeholder).
    pub fn empty(xrp_price: f64) -> Self {
        Self {
            total_value_usd: 0.0,
            impermanent_loss_pct: 0.0,
            impermanent_loss_usd: 0.0,
            delta_exposure_xrp: 0.0,
            delta_exposure_usd_if_down_10: 0.0,
            fee_income_7d: 0.0,
            current_xrp_price: xrp_price,
            sharpe_ratio: 0.0,
            var_95_usd: 0.0,
            break_even_lower: xrp_price,
            break_even_upper: xrp_price,
            fee_apr: 0.0,
            positions: Vec::new(),
            lending_vaults: Vec::new(),
            open_loans: Vec::new(),
            cvar_95_usd: 0.0,
            gamma_usd: 0.0,
            net_carry: 0.0,
        }
    }
}
