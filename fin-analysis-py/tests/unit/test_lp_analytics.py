"""Unit tests for lp/analytics.py.

Tests focus on ``_compute_snapshot`` (the inner loop step) because it
encapsulates all fee + IL + PnL logic and is callable without parquet files.
"""

from __future__ import annotations

from datetime import date

import pytest

from fabdl.core.tickmath import get_sqrt_ratio_at_tick
from fabdl.lp.analytics import (
    PositionSnapshot,
    _compute_snapshot,
    position_stats,
)
from fabdl.lp.fees import fee_growth_inside
from fabdl.lp.position import Position, amounts_from_liquidity

_Q128 = 1 << 128

# Realistic USDC/WETH ticks — entry ≈ $2 000/WETH, ±2 000 ticks ≈ ±20 %.
ENTRY_TICK = 200_311
TICK_LO = ENTRY_TICK - 2_000
TICK_HI = ENTRY_TICK + 2_000
LIQUIDITY = 10**20

ENTRY_SQRT = get_sqrt_ratio_at_tick(ENTRY_TICK)


def _make_pos() -> Position:
    return Position(tick_lower=TICK_LO, tick_upper=TICK_HI, liquidity=LIQUIDITY)


def _entry_amounts(pos: Position) -> tuple[float, float]:
    from fabdl.core.constants import TOKEN0_DECIMALS, TOKEN1_DECIMALS

    sa = get_sqrt_ratio_at_tick(pos.tick_lower)
    sb = get_sqrt_ratio_at_tick(pos.tick_upper)
    a0, a1 = amounts_from_liquidity(pos.liquidity, sa, sb, ENTRY_SQRT)
    return a0 / (10**TOKEN0_DECIMALS), a1 / (10**TOKEN1_DECIMALS)


def _entry_price(pos: Position) -> float:
    from fabdl.lp.analytics import _sqrt_x96_to_price_usdc_per_weth

    return _sqrt_x96_to_price_usdc_per_weth(ENTRY_SQRT)


# ------------------------------------------------------------------ basic snapshot


def test_snapshot_zero_fees_at_entry():
    """First snapshot (fgi_prev == fgi_now) earns exactly zero fees."""
    pos = _make_pos()
    fgg = (100 * _Q128, 50 * _Q128)
    fgo_lo = (10 * _Q128, 5 * _Q128)
    fgo_hi = (20 * _Q128, 15 * _Q128)
    fgi_entry = fee_growth_inside(
        ENTRY_TICK, pos.tick_lower, pos.tick_upper, fgo_lo, fgo_hi, fgg
    )
    ep = _entry_price(pos)
    ea0, ea1 = _entry_amounts(pos)

    snap, new_fgi, cf0, cf1 = _compute_snapshot(
        position=pos,
        day=date(2025, 10, 1),
        tick_current=ENTRY_TICK,
        sqrt_price_x96=ENTRY_SQRT,
        fee_growth_global=fgg,
        fgo_lower=fgo_lo,
        fgo_upper=fgo_hi,
        fgi_prev=fgi_entry,  # same as entry → zero delta
        cum_fees0=0.0,
        cum_fees1=0.0,
        entry_price=ep,
        entry_amount0=ea0,
        entry_amount1=ea1,
    )
    assert snap.fees_token0_usdc == pytest.approx(0.0, abs=1e-12)
    assert snap.fees_token1_weth == pytest.approx(0.0, abs=1e-12)
    assert snap.il_ratio == pytest.approx(0.0, abs=1e-6)
    assert snap.pnl_vs_hodl_usd == pytest.approx(0.0, abs=1e-6)


def test_snapshot_fees_accumulate_correctly():
    """Fees = L * Δfgi / 2^128, matching fees_earned() directly."""
    from fabdl.core.constants import TOKEN0_DECIMALS, TOKEN1_DECIMALS
    from fabdl.lp.fees import fees_earned

    pos = _make_pos()
    fgg_prev = (100 * _Q128, 50 * _Q128)
    fgo_lo = (10 * _Q128, 5 * _Q128)
    fgo_hi = (20 * _Q128, 15 * _Q128)
    fgi_entry = fee_growth_inside(
        ENTRY_TICK, pos.tick_lower, pos.tick_upper, fgo_lo, fgo_hi, fgg_prev
    )

    # Advance feeGrowthGlobal by 3 units each — all inside range
    fgg_now = (103 * _Q128, 53 * _Q128)
    fgi_now = fee_growth_inside(
        ENTRY_TICK, pos.tick_lower, pos.tick_upper, fgo_lo, fgo_hi, fgg_now
    )
    expected_raw0, expected_raw1 = fees_earned(pos.liquidity, fgi_now, fgi_entry)

    ep = _entry_price(pos)
    ea0, ea1 = _entry_amounts(pos)
    snap, _, cf0, cf1 = _compute_snapshot(
        position=pos,
        day=date(2025, 10, 2),
        tick_current=ENTRY_TICK,
        sqrt_price_x96=ENTRY_SQRT,
        fee_growth_global=fgg_now,
        fgo_lower=fgo_lo,
        fgo_upper=fgo_hi,
        fgi_prev=fgi_entry,
        cum_fees0=0.0,
        cum_fees1=0.0,
        entry_price=ep,
        entry_amount0=ea0,
        entry_amount1=ea1,
    )
    assert cf0 == pytest.approx(expected_raw0 / (10**TOKEN0_DECIMALS), rel=1e-9)
    assert cf1 == pytest.approx(expected_raw1 / (10**TOKEN1_DECIMALS), rel=1e-9)


def test_snapshot_il_negative_when_price_moves():
    """IL < 0 when price deviates from entry; HODL outperforms LP."""
    pos = _make_pos()
    # Move price down 10% from entry tick
    moved_tick = ENTRY_TICK - 1_000
    moved_sqrt = get_sqrt_ratio_at_tick(moved_tick)

    fgg = (100 * _Q128, 50 * _Q128)
    fgo_lo = (10 * _Q128, 5 * _Q128)
    fgo_hi = (20 * _Q128, 15 * _Q128)
    fgi_entry = fee_growth_inside(
        ENTRY_TICK, pos.tick_lower, pos.tick_upper, fgo_lo, fgo_hi, fgg
    )
    ep = _entry_price(pos)
    ea0, ea1 = _entry_amounts(pos)

    snap, _, _, _ = _compute_snapshot(
        position=pos,
        day=date(2025, 10, 5),
        tick_current=moved_tick,
        sqrt_price_x96=moved_sqrt,
        fee_growth_global=fgg,
        fgo_lower=fgo_lo,
        fgo_upper=fgo_hi,
        fgi_prev=fgi_entry,
        cum_fees0=0.0,
        cum_fees1=0.0,
        entry_price=ep,
        entry_amount0=ea0,
        entry_amount1=ea1,
    )
    assert snap.il_ratio < 0


def test_snapshot_position_below_range_all_token0():
    """When current price is below range, position holds only token0."""
    pos = _make_pos()
    below_tick = TICK_LO - 100
    below_sqrt = get_sqrt_ratio_at_tick(below_tick)

    fgg = (0, 0)
    fgi_entry = (0, 0)
    ep = _entry_price(pos)
    ea0, ea1 = _entry_amounts(pos)

    snap, _, _, _ = _compute_snapshot(
        position=pos,
        day=date(2025, 10, 5),
        tick_current=below_tick,
        sqrt_price_x96=below_sqrt,
        fee_growth_global=fgg,
        fgo_lower=(0, 0),
        fgo_upper=(0, 0),
        fgi_prev=fgi_entry,
        cum_fees0=0.0,
        cum_fees1=0.0,
        entry_price=ep,
        entry_amount0=ea0,
        entry_amount1=ea1,
    )
    assert snap.amount1_weth == pytest.approx(0.0, abs=1e-18)
    assert snap.amount0_usdc > 0


def test_snapshot_position_above_range_all_token1():
    """When current price is above range, position holds only token1."""
    pos = _make_pos()
    above_tick = TICK_HI + 100
    above_sqrt = get_sqrt_ratio_at_tick(above_tick)

    fgg = (0, 0)
    fgi_entry = (0, 0)
    ep = _entry_price(pos)
    ea0, ea1 = _entry_amounts(pos)

    snap, _, _, _ = _compute_snapshot(
        position=pos,
        day=date(2025, 10, 5),
        tick_current=above_tick,
        sqrt_price_x96=above_sqrt,
        fee_growth_global=fgg,
        fgo_lower=(0, 0),
        fgo_upper=(0, 0),
        fgi_prev=fgi_entry,
        cum_fees0=0.0,
        cum_fees1=0.0,
        entry_price=ep,
        entry_amount0=ea0,
        entry_amount1=ea1,
    )
    assert snap.amount0_usdc == pytest.approx(0.0, abs=1e-6)
    assert snap.amount1_weth > 0


# ------------------------------------------------------------------ position_stats


def test_position_stats_basic():
    """position_stats summarises correctly from a two-snapshot sequence."""
    pos = _make_pos()
    ep = _entry_price(pos)
    ea0, ea1 = _entry_amounts(pos)
    pv = ea0 + ea1 * ep

    snaps = [
        PositionSnapshot(
            date=date(2025, 10, 1),
            price_usdc_per_weth=ep,
            amount0_usdc=ea0,
            amount1_weth=ea1,
            position_value_usd=pv,
            fees_token0_usdc=0.0,
            fees_token1_weth=0.0,
            fees_usd=0.0,
            il_ratio=0.0,
            hodl_value_usd=pv,
            pnl_vs_hodl_usd=0.0,
        ),
        PositionSnapshot(
            date=date(2025, 10, 31),
            price_usdc_per_weth=ep,
            amount0_usdc=ea0,
            amount1_weth=ea1,
            position_value_usd=pv,
            fees_token0_usdc=100.0,
            fees_token1_weth=0.0,
            fees_usd=100.0,
            il_ratio=0.0,
            hodl_value_usd=pv,
            pnl_vs_hodl_usd=100.0,
        ),
    ]
    stats = position_stats(snaps, pos)
    assert stats.days == 31
    assert stats.total_fees_usd == pytest.approx(100.0)
    assert stats.il_ratio_final == pytest.approx(0.0)
    assert stats.total_pnl_usd == pytest.approx(100.0)
    assert stats.fee_apr > 0


def test_position_stats_single_snapshot():
    """Single-day snapshot: PnL and fees are zero; no crash."""
    pos = _make_pos()
    ep = _entry_price(pos)
    snaps = [
        PositionSnapshot(
            date=date(2025, 10, 1),
            price_usdc_per_weth=ep,
            amount0_usdc=1000.0,
            amount1_weth=0.5,
            position_value_usd=2000.0,
            fees_token0_usdc=0.0,
            fees_token1_weth=0.0,
            fees_usd=0.0,
            il_ratio=0.0,
            hodl_value_usd=2000.0,
            pnl_vs_hodl_usd=0.0,
        )
    ]
    stats = position_stats(snaps, pos)
    assert stats.days == 1
    assert stats.total_fees_usd == 0.0
