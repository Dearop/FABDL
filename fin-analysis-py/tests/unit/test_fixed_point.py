import pytest
from fabdl.core.fixed_point import UINT256_MAX, mul_div, mul_div_rounding_up


def test_mul_div_basic():
    assert mul_div(6, 7, 3) == 14
    assert mul_div(0, 10**30, 5) == 0


def test_mul_div_floors():
    assert mul_div(5, 1, 2) == 2  # 2.5 -> 2


def test_mul_div_rounding_up_no_remainder():
    assert mul_div_rounding_up(6, 7, 3) == 14


def test_mul_div_rounding_up_with_remainder():
    assert mul_div_rounding_up(5, 1, 2) == 3  # 2.5 -> 3


def test_mul_div_zero_denominator():
    with pytest.raises(ZeroDivisionError):
        mul_div(1, 1, 0)


def test_mul_div_uint256_overflow():
    with pytest.raises(OverflowError):
        mul_div(UINT256_MAX, 2, 1)


def test_mul_div_large_within_uint256():
    a = 1 << 200
    b = 1 << 55
    assert mul_div(a, b, 1 << 55) == a
