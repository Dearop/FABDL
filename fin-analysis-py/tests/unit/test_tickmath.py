import pytest
from fabdl.core.tickmath import (
    MAX_SQRT_RATIO,
    MAX_TICK,
    MIN_SQRT_RATIO,
    MIN_TICK,
    get_sqrt_ratio_at_tick,
    get_tick_at_sqrt_ratio,
)


def test_boundaries_match_v3_core():
    # Known constants from v3-core/contracts/libraries/TickMath.sol
    assert get_sqrt_ratio_at_tick(MIN_TICK) == MIN_SQRT_RATIO
    assert get_sqrt_ratio_at_tick(MAX_TICK) == MAX_SQRT_RATIO


def test_tick_zero_is_q96():
    # sqrt(1.0001^0) * 2^96 == 2^96
    assert get_sqrt_ratio_at_tick(0) == 1 << 96


def test_out_of_range_tick():
    with pytest.raises(ValueError):
        get_sqrt_ratio_at_tick(MAX_TICK + 1)


@pytest.mark.parametrize("tick", [-500_000, -1000, -10, -1, 1, 10, 1000, 500_000])
def test_roundtrip(tick: int):
    sqrt_x96 = get_sqrt_ratio_at_tick(tick)
    assert get_tick_at_sqrt_ratio(sqrt_x96) == tick
    # A price just below should give tick-1 (monotonicity check).
    assert get_tick_at_sqrt_ratio(sqrt_x96 - 1) == tick - 1


def test_tick_at_sqrt_ratio_bounds():
    with pytest.raises(ValueError):
        get_tick_at_sqrt_ratio(MIN_SQRT_RATIO - 1)
    with pytest.raises(ValueError):
        get_tick_at_sqrt_ratio(MAX_SQRT_RATIO)
