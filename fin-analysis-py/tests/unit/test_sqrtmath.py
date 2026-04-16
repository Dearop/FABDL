from fabdl.core.sqrtmath import (
    get_amount0_delta,
    get_amount1_delta,
    get_next_sqrt_price_from_input,
    get_next_sqrt_price_from_output,
)
from fabdl.core.tickmath import get_sqrt_ratio_at_tick


def test_amount_deltas_symmetric_in_arg_order():
    a = get_sqrt_ratio_at_tick(-1000)
    b = get_sqrt_ratio_at_tick(1000)
    L = 10**20
    assert get_amount0_delta(a, b, L, False) == get_amount0_delta(b, a, L, False)
    assert get_amount1_delta(a, b, L, False) == get_amount1_delta(b, a, L, False)


def test_rounding_directions():
    a = get_sqrt_ratio_at_tick(-100)
    b = get_sqrt_ratio_at_tick(100)
    L = 10**18
    assert get_amount0_delta(a, b, L, True) >= get_amount0_delta(a, b, L, False)
    assert get_amount1_delta(a, b, L, True) >= get_amount1_delta(a, b, L, False)


def test_zero_liquidity_gives_zero():
    a = get_sqrt_ratio_at_tick(-100)
    b = get_sqrt_ratio_at_tick(100)
    assert get_amount0_delta(a, b, 0, False) == 0
    assert get_amount1_delta(a, b, 0, False) == 0


def test_swap_direction_shifts_price():
    sqrt_p = get_sqrt_ratio_at_tick(0)
    L = 10**18
    amount = 10**16
    # zeroForOne swap (selling token0) drives price down.
    next_down = get_next_sqrt_price_from_input(sqrt_p, L, amount, True)
    assert next_down < sqrt_p
    # Selling token1 drives price up.
    next_up = get_next_sqrt_price_from_input(sqrt_p, L, amount, False)
    assert next_up > sqrt_p


def test_input_output_direction_consistent():
    """Input and output paths must move the price in the same direction."""
    sqrt_p = get_sqrt_ratio_at_tick(0)
    L = 10**20
    amount_in = 10**15
    next_p_in = get_next_sqrt_price_from_input(sqrt_p, L, amount_in, True)
    amount_out = get_amount1_delta(next_p_in, sqrt_p, L, False)
    next_p_out = get_next_sqrt_price_from_output(sqrt_p, L, amount_out, True)
    assert next_p_in < sqrt_p and next_p_out < sqrt_p
    # Output path rounds down, so its next price >= input path's next price (closer to sqrt_p).
    assert next_p_out >= next_p_in
