"""Pure-int port of Uniswap V3's ``UniswapV3Pool.swap`` outer loop.

Reads a ``PoolState`` snapshot (the set of initialized ticks and their
``liquidityNet``, plus ``slot0`` and active liquidity) and simulates a swap
tick-by-tick against the already-audited ``core.swapmath.compute_swap_step``.

Simplification vs the reference: this loop walks to the next *initialized*
tick instead of the next word boundary. Since uninitialized ticks cross with
``liquidityNet == 0`` (no state change), the two formulations produce
bit-identical amounts/fees/final-price for any completed swap.

No ``float``, no ``numpy``. Signed comparisons on ``sqrtPriceX96`` are done via
Python ``int``.
"""

from __future__ import annotations

import bisect
from dataclasses import dataclass, field, replace

from fabdl.core.swapmath import compute_swap_step
from fabdl.core.tickmath import (
    MAX_SQRT_RATIO,
    MAX_TICK,
    MIN_SQRT_RATIO,
    MIN_TICK,
    get_sqrt_ratio_at_tick,
    get_tick_at_sqrt_ratio,
)


@dataclass
class PoolState:
    sqrt_price_x96: int
    tick: int
    liquidity: int
    fee_pips: int                    # e.g. 500 for 0.05%
    initialized_ticks: list[int]     # sorted ascending
    liquidity_net: dict[int, int]    # tick -> signed int128

    def clone(self) -> PoolState:
        return PoolState(
            sqrt_price_x96=self.sqrt_price_x96,
            tick=self.tick,
            liquidity=self.liquidity,
            fee_pips=self.fee_pips,
            initialized_ticks=list(self.initialized_ticks),
            liquidity_net=dict(self.liquidity_net),
        )


@dataclass
class SwapResult:
    amount0: int              # signed, pool-perspective (positive = pool received token0)
    amount1: int
    sqrt_price_x96_end: int
    tick_end: int
    liquidity_end: int
    steps: int = 0
    fee_token_in: int = 0     # total fee taken in the input token (raw units)
    log: list[dict] = field(default_factory=list)  # debugging


def _next_initialized_tick(
    ticks: list[int], current_tick: int, zero_for_one: bool
) -> tuple[int, bool]:
    """Return (next_tick, initialized). If no initialized tick in the direction, clamp to MIN/MAX."""
    if zero_for_one:
        # largest initialized tick with value <= current_tick
        idx = bisect.bisect_right(ticks, current_tick) - 1
        if idx < 0:
            return MIN_TICK, False
        return ticks[idx], True
    # smallest initialized tick with value > current_tick
    idx = bisect.bisect_right(ticks, current_tick)
    if idx >= len(ticks):
        return MAX_TICK, False
    return ticks[idx], True


def simulate_swap(
    state: PoolState,
    amount_specified: int,
    zero_for_one: bool,
    sqrt_price_limit_x96: int | None = None,
    *,
    keep_log: bool = False,
) -> SwapResult:
    """Simulate a Uniswap V3 swap. Returns signed ``(amount0, amount1)`` from pool's perspective.

    Positive amount = pool received; negative = pool sent out. Mirrors the sign
    convention of the ``Swap`` event.
    """
    if amount_specified == 0:
        return SwapResult(0, 0, state.sqrt_price_x96, state.tick, state.liquidity)

    if sqrt_price_limit_x96 is None:
        sqrt_price_limit_x96 = (MIN_SQRT_RATIO + 1) if zero_for_one else (MAX_SQRT_RATIO - 1)

    # Validate limit is on the correct side of current price.
    if zero_for_one:
        assert MIN_SQRT_RATIO < sqrt_price_limit_x96 < state.sqrt_price_x96
    else:
        assert state.sqrt_price_x96 < sqrt_price_limit_x96 < MAX_SQRT_RATIO

    exact_input = amount_specified > 0
    amount_remaining = amount_specified
    amount_calculated = 0
    sqrt_price = state.sqrt_price_x96
    tick = state.tick
    liquidity = state.liquidity
    total_fee = 0
    log: list[dict] = []

    steps = 0
    while amount_remaining != 0 and sqrt_price != sqrt_price_limit_x96:
        tick_next, initialized = _next_initialized_tick(state.initialized_ticks, tick, zero_for_one)
        tick_next = max(MIN_TICK, min(MAX_TICK, tick_next))
        sqrt_price_next = get_sqrt_ratio_at_tick(tick_next)

        if zero_for_one:
            target = max(sqrt_price_next, sqrt_price_limit_x96)
        else:
            target = min(sqrt_price_next, sqrt_price_limit_x96)

        sqrt_price, amount_in, amount_out, fee_amount = compute_swap_step(
            sqrt_price, target, liquidity, amount_remaining, state.fee_pips
        )

        if exact_input:
            amount_remaining -= amount_in + fee_amount
            amount_calculated -= amount_out
        else:
            amount_remaining += amount_out
            amount_calculated += amount_in + fee_amount
        total_fee += fee_amount

        if keep_log:
            log.append(
                {
                    "step": steps,
                    "sqrt_price": sqrt_price,
                    "tick_next": tick_next,
                    "amount_in": amount_in,
                    "amount_out": amount_out,
                    "fee": fee_amount,
                    "liquidity": liquidity,
                }
            )

        if sqrt_price == sqrt_price_next:
            # crossed the tick boundary
            if initialized:
                net = state.liquidity_net.get(tick_next, 0)
                if zero_for_one:
                    net = -net
                liquidity = liquidity + net
            tick = tick_next - 1 if zero_for_one else tick_next
        elif sqrt_price != state.sqrt_price_x96:
            tick = get_tick_at_sqrt_ratio(sqrt_price)

        steps += 1

    # Convert (amountSpecifiedRemaining, amountCalculated) into (amount0, amount1) per Solidity.
    if zero_for_one == exact_input:
        amount0 = amount_specified - amount_remaining
        amount1 = amount_calculated
    else:
        amount0 = amount_calculated
        amount1 = amount_specified - amount_remaining

    return SwapResult(
        amount0=amount0,
        amount1=amount1,
        sqrt_price_x96_end=sqrt_price,
        tick_end=tick,
        liquidity_end=liquidity,
        steps=steps,
        fee_token_in=total_fee,
        log=log,
    )


def apply_swap(state: PoolState, result: SwapResult) -> PoolState:
    """Return a new PoolState updated to post-swap sqrt_price / tick / liquidity."""
    return replace(
        state,
        sqrt_price_x96=result.sqrt_price_x96_end,
        tick=result.tick_end,
        liquidity=result.liquidity_end,
    )
