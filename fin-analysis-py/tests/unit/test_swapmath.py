from fabdl.core.swapmath import compute_swap_step
from fabdl.core.tickmath import get_sqrt_ratio_at_tick


def test_exact_input_consumes_fee():
    sqrt_cur = get_sqrt_ratio_at_tick(0)
    sqrt_tgt = get_sqrt_ratio_at_tick(-100)  # zeroForOne, price moves down
    L = 10**20
    amount_in = 10**15
    fee_pips = 500

    next_p, ain, aout, fee = compute_swap_step(sqrt_cur, sqrt_tgt, L, amount_in, fee_pips)
    assert next_p < sqrt_cur           # price moved down
    assert ain + fee <= amount_in      # fee is carved out of input
    assert aout > 0


def test_exact_output_caps_amount_out():
    sqrt_cur = get_sqrt_ratio_at_tick(0)
    sqrt_tgt = get_sqrt_ratio_at_tick(-10_000)
    L = 10**20
    amount_out = 10**12

    _, _, aout, _ = compute_swap_step(sqrt_cur, sqrt_tgt, L, -amount_out, 500)
    assert aout <= amount_out


def test_target_reached_on_large_input():
    sqrt_cur = get_sqrt_ratio_at_tick(0)
    sqrt_tgt = get_sqrt_ratio_at_tick(-100)
    L = 10**18
    huge_input = 10**30

    next_p, _, _, _ = compute_swap_step(sqrt_cur, sqrt_tgt, L, huge_input, 500)
    assert next_p == sqrt_tgt
