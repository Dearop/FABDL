from __future__ import annotations

from fabdl.lp.il import break_even_prices, il_curve

# Realistic USDC/WETH ticks: human=2000 ↔ raw=5e-16 ↔ tick≈-352337.
# ±2000 ticks ≈ ±20% price move — ample for IL tests.
ENTRY_TICK = 200311
LO_TICK = ENTRY_TICK - 2000
HI_TICK = ENTRY_TICK + 2000


def test_il_curve_zero_at_entry():
    curve = il_curve(
        liquidity=10**20,
        tick_lower=LO_TICK,
        tick_upper=HI_TICK,
        entry_price_usdc_per_weth=2000.0,
        price_grid_usdc_per_weth=[2000.0],
    )
    assert abs(curve[0].il_ratio) < 1e-3


def test_il_curve_negative_when_price_moves():
    curve = il_curve(
        liquidity=10**20,
        tick_lower=LO_TICK,
        tick_upper=HI_TICK,
        entry_price_usdc_per_weth=2000.0,
        price_grid_usdc_per_weth=[1800.0, 2200.0],
    )
    for pt in curve:
        assert pt.il_ratio <= 1e-6


def test_break_even_symmetric_with_fees():
    # Fees ≈ 1% of position value → break-even band well inside the range.
    lo, hi = break_even_prices(
        liquidity=10**20,
        tick_lower=LO_TICK,
        tick_upper=HI_TICK,
        entry_price_usdc_per_weth=2000.0,
        fees_earned_usd=8e6,
        search_span=0.2,
    )
    assert lo is not None and hi is not None
    assert lo < 2000.0 < hi


def test_break_even_none_without_fees():
    # Zero fees → IL is ≤ 0 everywhere; no positive sign change.
    lo, hi = break_even_prices(
        liquidity=10**20,
        tick_lower=LO_TICK,
        tick_upper=HI_TICK,
        entry_price_usdc_per_weth=2000.0,
        fees_earned_usd=0.0,
        search_span=0.1,
    )
    assert lo is None and hi is None
