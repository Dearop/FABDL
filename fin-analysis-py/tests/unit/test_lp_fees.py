from __future__ import annotations

from fabdl.lp.fees import fee_apr, fee_growth_inside, fees_earned

_Q128 = 1 << 128


def test_fee_growth_inside_price_inside():
    fgg = (100 * _Q128, 50 * _Q128)
    fgo_lower = (10 * _Q128, 5 * _Q128)
    fgo_upper = (20 * _Q128, 15 * _Q128)
    fgi = fee_growth_inside(
        current_tick=0,
        tick_lower=-10,
        tick_upper=10,
        fgo_lower=fgo_lower,
        fgo_upper=fgo_upper,
        fee_growth_global=fgg,
    )
    # inside: fgi = global - fgo_lower - fgo_upper
    assert fgi == ((100 - 10 - 20) * _Q128, (50 - 5 - 15) * _Q128)


def test_fee_growth_inside_price_below_range():
    fgg = (100 * _Q128, 0)
    fgo_lower = (10 * _Q128, 0)
    fgo_upper = (20 * _Q128, 0)
    fgi = fee_growth_inside(
        current_tick=-100,
        tick_lower=-10,
        tick_upper=10,
        fgo_lower=fgo_lower,
        fgo_upper=fgo_upper,
        fee_growth_global=fgg,
    )
    # below: below = gg - fgo_lower, above = fgo_upper
    # fgi = gg - (gg - fgo_lower) - fgo_upper = fgo_lower - fgo_upper  (mod 2^256)
    assert fgi[0] == ((10 - 20) * _Q128) % (1 << 256)


def test_fees_earned_linear_in_liquidity():
    fgi_last = (0, 0)
    fgi_now = (5 * _Q128, 7 * _Q128)
    f1 = fees_earned(10**18, fgi_now, fgi_last)
    f2 = fees_earned(2 * 10**18, fgi_now, fgi_last)
    assert f2[0] == 2 * f1[0]
    assert f2[1] == 2 * f1[1]
    assert f1 == (5 * 10**18, 7 * 10**18)


def test_fee_apr_annualises():
    # $10 fees on $1000 over 1 day → APR = 3.65
    apr = fee_apr(10.0, 1000.0, 86400.0)
    assert abs(apr - 3.65) < 1e-9
