/// Delta exposure calculation.
///
/// Estimates the sensitivity of an LP position to XRP price movements.
/// For a 50/50 constant-product AMM, approximately half the position value
/// is in XRP.

/// Compute the XRP delta exposure of a position (in XRP units).
///
/// `position_value_usd` — current USD value of the LP position.
/// `xrp_price_usd`      — current XRP spot price.
///
/// Returns the number of XRP the position is effectively long.
/// Positive → gains when XRP price rises.
pub fn delta_xrp(position_value_usd: f64, xrp_price_usd: f64) -> f64 {
    if xrp_price_usd <= 0.0 {
        return 0.0;
    }
    // 50/50 assumption for constant-product AMM: half the value is in XRP.
    0.5 * position_value_usd / xrp_price_usd
}

/// Change in portfolio USD value if XRP price moves by `price_move_pct`.
///
/// `price_move_pct` — signed fraction, e.g. -0.10 for a 10 % drop.
pub fn delta_usd_for_move(delta_xrp_amount: f64, xrp_price_usd: f64, price_move_pct: f64) -> f64 {
    delta_xrp_amount * xrp_price_usd * price_move_pct
}

/// Convenience: USD P&L if XRP drops 10 %.
pub fn delta_usd_if_down_10(delta_xrp_amount: f64, xrp_price_usd: f64) -> f64 {
    delta_usd_for_move(delta_xrp_amount, xrp_price_usd, -0.10)
}
