"""V3 fee-growth accounting per ``LP_FORMULAS.md``.

``fee_growth_inside_{0,1}`` for a position is the global accumulator minus the
``feeGrowthOutside`` at the lower and upper ticks, conditional on the current
tick's position relative to the range. These values are Q128 fixed-point
uint256 cumulatives; unsigned subtraction under mod 2^256 is intentional.

Given two snapshots of ``fee_growth_inside`` the fees earned by a position of
constant ``L`` are ``L * (fgi_now - fgi_last) / 2**128`` per token.
"""

from __future__ import annotations

from dataclasses import dataclass

_Q128 = 1 << 128
_UINT256 = 1 << 256


def _u256_sub(a: int, b: int) -> int:
    """Two's-complement uint256 subtraction, matching Solidity overflow semantics."""
    return (a - b) % _UINT256


@dataclass(frozen=True)
class TickFeeGrowth:
    tick: int
    fee_growth_outside_0: int
    fee_growth_outside_1: int


def fee_growth_inside(
    current_tick: int,
    tick_lower: int,
    tick_upper: int,
    fgo_lower: tuple[int, int],
    fgo_upper: tuple[int, int],
    fee_growth_global: tuple[int, int],
) -> tuple[int, int]:
    """Return ``(fgi_0, fgi_1)`` per UniswapV3Pool._updatePosition."""
    out = []
    for i in (0, 1):
        below = fgo_lower[i] if current_tick >= tick_lower else _u256_sub(
            fee_growth_global[i], fgo_lower[i]
        )
        above = fgo_upper[i] if current_tick < tick_upper else _u256_sub(
            fee_growth_global[i], fgo_upper[i]
        )
        out.append(_u256_sub(_u256_sub(fee_growth_global[i], below), above))
    return out[0], out[1]


def fees_earned(
    liquidity: int,
    fgi_now: tuple[int, int],
    fgi_last: tuple[int, int],
) -> tuple[int, int]:
    """``(fees_token0, fees_token1)`` in raw units. uint256 mod subtract."""
    d0 = _u256_sub(fgi_now[0], fgi_last[0])
    d1 = _u256_sub(fgi_now[1], fgi_last[1])
    return (liquidity * d0) // _Q128, (liquidity * d1) // _Q128


def fees_earned_usd(
    fees0: int, fees1: int, weth_price_usd: float
) -> float:
    """USDC/WETH-specific USD value of earned fees."""
    from fabdl.core.constants import TOKEN0_DECIMALS, TOKEN1_DECIMALS
    return fees0 / (10**TOKEN0_DECIMALS) + (fees1 / (10**TOKEN1_DECIMALS)) * weth_price_usd


def fee_apr(fees_usd: float, position_value_usd: float, window_seconds: float) -> float:
    """Annualised simple fee APR. Undefined / 0 for empty position or zero window."""
    if position_value_usd <= 0 or window_seconds <= 0:
        return 0.0
    return (fees_usd / position_value_usd) * (365.0 * 24 * 3600 / window_seconds)
