"""LP position math: amounts <-> liquidity, piecewise by current price.

Pure-int against the V3 sqrt-math. A position is defined by a tick range
``[tick_lower, tick_upper)`` and a virtual ``liquidity`` ``L``. At any current
``sqrtPriceX96`` the tokens it holds are:

- below range  (``P <= P_lower``): all token0, ``amount0 = L*(√P_u−√P_l)/(√P_u·√P_l) * Q96``
- inside range (``P_lower < P < P_upper``): both
- above range  (``P >= P_upper``): all token1, ``amount1 = L*(√P_u−√P_l) / Q96``
"""

from __future__ import annotations

from dataclasses import dataclass

from fabdl.core.sqrtmath import get_amount0_delta, get_amount1_delta
from fabdl.core.tickmath import get_sqrt_ratio_at_tick


@dataclass(frozen=True)
class Position:
    tick_lower: int
    tick_upper: int
    liquidity: int


def amounts_from_liquidity(
    liquidity: int,
    sqrt_lower_x96: int,
    sqrt_upper_x96: int,
    sqrt_current_x96: int,
) -> tuple[int, int]:
    """Return ``(amount0, amount1)`` in raw token units (rounded down)."""
    if sqrt_lower_x96 > sqrt_upper_x96:
        sqrt_lower_x96, sqrt_upper_x96 = sqrt_upper_x96, sqrt_lower_x96
    if sqrt_current_x96 <= sqrt_lower_x96:
        return (
            get_amount0_delta(sqrt_lower_x96, sqrt_upper_x96, liquidity, False),
            0,
        )
    if sqrt_current_x96 >= sqrt_upper_x96:
        return (
            0,
            get_amount1_delta(sqrt_lower_x96, sqrt_upper_x96, liquidity, False),
        )
    return (
        get_amount0_delta(sqrt_current_x96, sqrt_upper_x96, liquidity, False),
        get_amount1_delta(sqrt_lower_x96, sqrt_current_x96, liquidity, False),
    )


def amounts_from_liquidity_at_tick(
    liquidity: int, tick_lower: int, tick_upper: int, tick_current: int
) -> tuple[int, int]:
    return amounts_from_liquidity(
        liquidity,
        get_sqrt_ratio_at_tick(tick_lower),
        get_sqrt_ratio_at_tick(tick_upper),
        get_sqrt_ratio_at_tick(tick_current),
    )


def liquidity_for_amount0(
    sqrt_a_x96: int, sqrt_b_x96: int, amount0: int
) -> int:
    """Max liquidity such that the position holds ``<= amount0`` token0 (below range)."""
    if sqrt_a_x96 > sqrt_b_x96:
        sqrt_a_x96, sqrt_b_x96 = sqrt_b_x96, sqrt_a_x96
    # L = amount0 * (sqrt_a * sqrt_b) / (Q96 * (sqrt_b - sqrt_a))
    from fabdl.core.constants import Q96
    return (amount0 * sqrt_a_x96 * sqrt_b_x96) // (Q96 * (sqrt_b_x96 - sqrt_a_x96))


def liquidity_for_amount1(
    sqrt_a_x96: int, sqrt_b_x96: int, amount1: int
) -> int:
    """Max liquidity such that the position holds ``<= amount1`` token1 (above range)."""
    if sqrt_a_x96 > sqrt_b_x96:
        sqrt_a_x96, sqrt_b_x96 = sqrt_b_x96, sqrt_a_x96
    from fabdl.core.constants import Q96
    return (amount1 * Q96) // (sqrt_b_x96 - sqrt_a_x96)


def liquidity_for_amounts(
    sqrt_current_x96: int,
    sqrt_a_x96: int,
    sqrt_b_x96: int,
    amount0: int,
    amount1: int,
) -> int:
    """Maximum liquidity supplied by (amount0, amount1) at the current price."""
    if sqrt_a_x96 > sqrt_b_x96:
        sqrt_a_x96, sqrt_b_x96 = sqrt_b_x96, sqrt_a_x96
    if sqrt_current_x96 <= sqrt_a_x96:
        return liquidity_for_amount0(sqrt_a_x96, sqrt_b_x96, amount0)
    if sqrt_current_x96 >= sqrt_b_x96:
        return liquidity_for_amount1(sqrt_a_x96, sqrt_b_x96, amount1)
    l0 = liquidity_for_amount0(sqrt_current_x96, sqrt_b_x96, amount0)
    l1 = liquidity_for_amount1(sqrt_a_x96, sqrt_current_x96, amount1)
    return min(l0, l1)


def position_value_usd(
    liquidity: int,
    tick_lower: int,
    tick_upper: int,
    sqrt_current_x96: int,
    weth_price_usd: float,
) -> float:
    """USD value at current price. USDC/WETH-specific (token0=USDC 6dec, token1=WETH 18dec)."""
    from fabdl.core.constants import TOKEN0_DECIMALS, TOKEN1_DECIMALS
    a0, a1 = amounts_from_liquidity(
        liquidity,
        get_sqrt_ratio_at_tick(tick_lower),
        get_sqrt_ratio_at_tick(tick_upper),
        sqrt_current_x96,
    )
    return a0 / (10**TOKEN0_DECIMALS) + (a1 / (10**TOKEN1_DECIMALS)) * weth_price_usd
