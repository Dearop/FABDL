"""Simulator correctness on synthetic pool states.

The bit-exact parity test against real on-chain swaps lives in
``test_swap_vs_onchain.py`` and requires a live RPC endpoint — those tests are
skipped unless ``RPC_URL`` is set.
"""

from __future__ import annotations

from fabdl.core.tickmath import get_sqrt_ratio_at_tick
from fabdl.simulate.swap import PoolState, simulate_swap


def _single_position_state(
    tick_lower: int, tick_upper: int, L: int, current_tick: int = 0, fee_pips: int = 500
) -> PoolState:
    return PoolState(
        sqrt_price_x96=get_sqrt_ratio_at_tick(current_tick),
        tick=current_tick,
        liquidity=L,
        fee_pips=fee_pips,
        initialized_ticks=sorted([tick_lower, tick_upper]),
        liquidity_net={tick_lower: L, tick_upper: -L},
    )


def test_zero_input_is_noop():
    s = _single_position_state(-1000, 1000, 10**20)
    r = simulate_swap(s, 0, zero_for_one=True)
    assert r.amount0 == 0 and r.amount1 == 0
    assert r.sqrt_price_x96_end == s.sqrt_price_x96


def test_zero_for_one_moves_price_down():
    s = _single_position_state(-1000, 1000, 10**22)
    r = simulate_swap(s, 10**15, zero_for_one=True)
    assert r.amount0 > 0        # pool received token0
    assert r.amount1 < 0        # pool sent token1
    assert r.sqrt_price_x96_end < s.sqrt_price_x96


def test_one_for_zero_moves_price_up():
    s = _single_position_state(-1000, 1000, 10**22)
    r = simulate_swap(s, 10**15, zero_for_one=False)
    assert r.amount0 < 0
    assert r.amount1 > 0
    assert r.sqrt_price_x96_end > s.sqrt_price_x96


def test_fee_is_nonzero_and_carved_from_input():
    L = 10**22
    s = _single_position_state(-1000, 1000, L)
    amt_in = 10**15
    r = simulate_swap(s, amt_in, zero_for_one=True)
    assert r.fee_token_in > 0
    # In V3 the pool's `amount0` on an exact-input swap equals the full gross
    # input (user paid amt_in inclusive of fee).
    assert r.amount0 == amt_in


def test_exact_output_swap():
    s = _single_position_state(-1000, 1000, 10**22)
    desired_out = 10**12
    r = simulate_swap(s, -desired_out, zero_for_one=True)
    # pool sent desired_out of token1 (amount1 negative, magnitude >= desired_out
    # up to rounding within 1 wei)
    assert r.amount1 <= -desired_out
    assert r.amount0 > 0


def test_crossing_initialized_tick_changes_active_liquidity():
    """Two overlapping positions. Moving price past the inner tick drops L."""
    L1 = 10**18    # wide outer position
    L2 = 10**21    # narrow inner position
    inner_lo, inner_hi = -10, 10
    outer_lo, outer_hi = -1000, 1000

    state = PoolState(
        sqrt_price_x96=get_sqrt_ratio_at_tick(0),
        tick=0,
        liquidity=L1 + L2,
        fee_pips=500,
        initialized_ticks=sorted([outer_lo, inner_lo, inner_hi, outer_hi]),
        liquidity_net={outer_lo: L1, inner_lo: L2, inner_hi: -L2, outer_hi: -L1},
    )
    # Input large enough to cross inner_lo but bounded by a sqrt-price limit
    # inside the outer range so we don't blow past outer_lo.
    limit = get_sqrt_ratio_at_tick(-100)
    r = simulate_swap(state, 10**19, zero_for_one=True, sqrt_price_limit_x96=limit, keep_log=True)
    assert r.tick_end < inner_lo
    assert r.tick_end > outer_lo
    assert r.liquidity_end == L1  # after crossing inner_lo going down, L2 removed
    assert r.steps >= 2            # at least one tick crossing + continuation
