from __future__ import annotations

from fabdl.core.tickmath import get_sqrt_ratio_at_tick
from fabdl.lp.position import (
    amounts_from_liquidity,
    liquidity_for_amounts,
)


def test_below_range_all_token0():
    sa, sb = get_sqrt_ratio_at_tick(100), get_sqrt_ratio_at_tick(200)
    sp = get_sqrt_ratio_at_tick(50)
    a0, a1 = amounts_from_liquidity(10**18, sa, sb, sp)
    assert a0 > 0 and a1 == 0


def test_above_range_all_token1():
    sa, sb = get_sqrt_ratio_at_tick(100), get_sqrt_ratio_at_tick(200)
    sp = get_sqrt_ratio_at_tick(300)
    a0, a1 = amounts_from_liquidity(10**18, sa, sb, sp)
    assert a0 == 0 and a1 > 0


def test_inside_range_both_nonzero():
    sa, sb = get_sqrt_ratio_at_tick(-100), get_sqrt_ratio_at_tick(100)
    sp = get_sqrt_ratio_at_tick(0)
    a0, a1 = amounts_from_liquidity(10**18, sa, sb, sp)
    assert a0 > 0 and a1 > 0


def test_liquidity_roundtrip_inside_range():
    sa, sb = get_sqrt_ratio_at_tick(-500), get_sqrt_ratio_at_tick(500)
    sp = get_sqrt_ratio_at_tick(0)
    L_orig = 10**20
    a0, a1 = amounts_from_liquidity(L_orig, sa, sb, sp)
    L_recovered = liquidity_for_amounts(sp, sa, sb, a0, a1)
    # Allow small rounding loss (<= ~Q96/range rounding per side)
    assert abs(L_recovered - L_orig) / L_orig < 1e-6
