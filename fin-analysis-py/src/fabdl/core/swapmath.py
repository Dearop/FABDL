"""Port of Uniswap v3-core SwapMath.sol.

``compute_swap_step`` returns the (next_sqrt_price, amount_in, amount_out, fee)
for a single tick-range step of a Uniswap V3 swap. Pure-int.

Assumption for this project: pool protocolFee = 0. Flash loans are not modelled
(the pool under study does not have Flash callers in the grading window in any
way that affects swap pricing).
"""

from fabdl.core.fixed_point import mul_div, mul_div_rounding_up
from fabdl.core.sqrtmath import (
    get_amount0_delta,
    get_amount1_delta,
    get_next_sqrt_price_from_input,
    get_next_sqrt_price_from_output,
)


def compute_swap_step(
    sqrt_ratio_current_x96: int,
    sqrt_ratio_target_x96: int,
    liquidity: int,
    amount_remaining: int,
    fee_pips: int,
) -> tuple[int, int, int, int]:
    """Compute the swap within a single initialized tick range.

    Args:
        sqrt_ratio_current_x96: current pool sqrt price (Q64.96).
        sqrt_ratio_target_x96: target sqrt price — next initialized tick or the user's price limit.
        liquidity: active liquidity over the range.
        amount_remaining: signed amount still to swap (positive = exact input, negative = exact output).
        fee_pips: fee as parts-per-million (e.g. 500 for 0.05%).

    Returns:
        (next_sqrt_ratio_x96, amount_in, amount_out, fee_amount), all unsigned.
    """
    zero_for_one = sqrt_ratio_current_x96 >= sqrt_ratio_target_x96
    exact_in = amount_remaining >= 0

    if exact_in:
        amount_remaining_less_fee = mul_div(amount_remaining, 10**6 - fee_pips, 10**6)
        if zero_for_one:
            amount_in = get_amount0_delta(
                sqrt_ratio_target_x96, sqrt_ratio_current_x96, liquidity, True
            )
        else:
            amount_in = get_amount1_delta(
                sqrt_ratio_current_x96, sqrt_ratio_target_x96, liquidity, True
            )
        if amount_remaining_less_fee >= amount_in:
            sqrt_ratio_next_x96 = sqrt_ratio_target_x96
        else:
            sqrt_ratio_next_x96 = get_next_sqrt_price_from_input(
                sqrt_ratio_current_x96, liquidity, amount_remaining_less_fee, zero_for_one
            )
    else:
        if zero_for_one:
            amount_out = get_amount1_delta(
                sqrt_ratio_target_x96, sqrt_ratio_current_x96, liquidity, False
            )
        else:
            amount_out = get_amount0_delta(
                sqrt_ratio_current_x96, sqrt_ratio_target_x96, liquidity, False
            )
        if -amount_remaining >= amount_out:
            sqrt_ratio_next_x96 = sqrt_ratio_target_x96
        else:
            sqrt_ratio_next_x96 = get_next_sqrt_price_from_output(
                sqrt_ratio_current_x96, liquidity, -amount_remaining, zero_for_one
            )

    max_reached = sqrt_ratio_target_x96 == sqrt_ratio_next_x96

    if zero_for_one:
        amount_in = (
            amount_in
            if max_reached and exact_in
            else get_amount0_delta(
                sqrt_ratio_next_x96, sqrt_ratio_current_x96, liquidity, True
            )
        )
        amount_out = (
            get_amount1_delta(
                sqrt_ratio_next_x96, sqrt_ratio_current_x96, liquidity, False
            )
            if not (max_reached and not exact_in)
            else amount_out  # type: ignore[has-type]
        )
    else:
        amount_in = (
            amount_in
            if max_reached and exact_in
            else get_amount1_delta(
                sqrt_ratio_current_x96, sqrt_ratio_next_x96, liquidity, True
            )
        )
        amount_out = (
            get_amount0_delta(
                sqrt_ratio_current_x96, sqrt_ratio_next_x96, liquidity, False
            )
            if not (max_reached and not exact_in)
            else amount_out  # type: ignore[has-type]
        )

    # Cap output for exact-output swaps.
    if not exact_in and amount_out > -amount_remaining:
        amount_out = -amount_remaining

    if exact_in and sqrt_ratio_next_x96 != sqrt_ratio_target_x96:
        fee_amount = amount_remaining - amount_in
    else:
        fee_amount = mul_div_rounding_up(amount_in, fee_pips, 10**6 - fee_pips)

    return sqrt_ratio_next_x96, amount_in, amount_out, fee_amount
