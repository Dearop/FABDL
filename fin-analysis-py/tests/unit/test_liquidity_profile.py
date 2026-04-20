from __future__ import annotations

from decimal import Decimal

import polars as pl

from fabdl.core.tickmath import get_sqrt_ratio_at_tick
from fabdl.liquidity.concentration import effective_tick_range, hhi
from fabdl.liquidity.profile import RangeTVL, tick_ranges, tvl_decomposition


def _snapshot(ticks_nets: list[tuple[int, int]]) -> pl.DataFrame:
    return pl.DataFrame(
        {
            "tick": [t for t, _ in ticks_nets],
            "liquidity_net": [Decimal(n) for _, n in ticks_nets],
            "liquidity_gross": [Decimal(abs(n)) for _, n in ticks_nets],
        }
    )


def test_tick_ranges_running_active_liquidity():
    snap = _snapshot([(-10, 1000), (10, -1000)])
    ranges = tick_ranges(snap)
    assert len(ranges) == 1
    assert ranges[0].tick_lower == -10
    assert ranges[0].tick_upper == 10
    assert ranges[0].active_liquidity == 1000


def test_tick_ranges_overlapping():
    snap = _snapshot([(-20, 500), (-10, 1000), (10, -1000), (20, -500)])
    ranges = tick_ranges(snap)
    # [-20..-10] = 500, [-10..10] = 1500, [10..20] = 500
    assert [r.active_liquidity for r in ranges] == [500, 1500, 500]


def test_tvl_decomposition_price_inside():
    snap = _snapshot([(-10, 10**18), (10, -(10**18))])
    sqrt_p = get_sqrt_ratio_at_tick(0)
    decomp = tvl_decomposition(snap, sqrt_p, weth_price_usd=2000.0)
    assert len(decomp) == 1
    # Price inside → both amounts nonzero.
    assert decomp[0].amount0_raw > 0
    assert decomp[0].amount1_raw > 0
    assert decomp[0].usd_value > 0


def test_tvl_decomposition_price_above_range_all_token1():
    snap = _snapshot([(-10, 10**18), (10, -(10**18))])
    sqrt_p = get_sqrt_ratio_at_tick(100)  # above the range
    decomp = tvl_decomposition(snap, sqrt_p, weth_price_usd=2000.0)
    assert decomp[0].amount0_raw == 0
    assert decomp[0].amount1_raw > 0


def test_tvl_decomposition_price_below_range_all_token0():
    snap = _snapshot([(-10, 10**18), (10, -(10**18))])
    sqrt_p = get_sqrt_ratio_at_tick(-100)
    decomp = tvl_decomposition(snap, sqrt_p, weth_price_usd=2000.0)
    assert decomp[0].amount0_raw > 0
    assert decomp[0].amount1_raw == 0


def test_hhi_uniform_vs_concentrated():
    uniform = [
        RangeTVL(0, 10, 0, 0, 0, 0.0, 0.0, 100.0),
        RangeTVL(10, 20, 0, 0, 0, 0.0, 0.0, 100.0),
        RangeTVL(20, 30, 0, 0, 0, 0.0, 0.0, 100.0),
        RangeTVL(30, 40, 0, 0, 0, 0.0, 0.0, 100.0),
    ]
    concentrated = [
        RangeTVL(0, 10, 0, 0, 0, 0.0, 0.0, 1.0),
        RangeTVL(10, 20, 0, 0, 0, 0.0, 0.0, 1.0),
        RangeTVL(20, 30, 0, 0, 0, 0.0, 0.0, 397.0),
        RangeTVL(30, 40, 0, 0, 0, 0.0, 0.0, 1.0),
    ]
    assert hhi(uniform) < hhi(concentrated)
    assert abs(hhi(uniform) - 0.25) < 1e-9  # 4 equal buckets → 4*(0.25)^2


def test_effective_tick_range_covers_target():
    decomp = [
        RangeTVL(0, 10, 0, 0, 0, 0.0, 0.0, 1.0),
        RangeTVL(10, 20, 0, 0, 0, 0.0, 0.0, 90.0),
        RangeTVL(20, 30, 0, 0, 0, 0.0, 0.0, 6.0),
        RangeTVL(30, 40, 0, 0, 0, 0.0, 0.0, 3.0),
    ]
    # 90 alone < 95% of 100. Narrowest covering ≥95 is [10..30] (90+6=96).
    rng = effective_tick_range(decomp, fraction=0.95)
    assert rng == (10, 30)
