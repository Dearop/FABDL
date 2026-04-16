"""Port of Uniswap v3-core SqrtPriceMath.sol.

Computes token amounts from sqrt-price and liquidity, and next sqrt-price from an
input or output amount. Pure-int, line-auditable against the Solidity source.
"""

from fabdl.core.constants import Q96
from fabdl.core.fixed_point import mul_div, mul_div_rounding_up

_UINT160_MAX = (1 << 160) - 1
_UINT256_MAX = (1 << 256) - 1


def _require_uint160(x: int) -> int:
    if x < 0 or x > _UINT160_MAX:
        raise OverflowError("sqrtPriceX96 does not fit in uint160")
    return x


def get_amount0_delta(
    sqrt_ratio_a_x96: int,
    sqrt_ratio_b_x96: int,
    liquidity: int,
    round_up: bool,
) -> int:
    """|Δtoken0| for a liquidity position between two prices (Solidity: getAmount0Delta)."""
    if sqrt_ratio_a_x96 > sqrt_ratio_b_x96:
        sqrt_ratio_a_x96, sqrt_ratio_b_x96 = sqrt_ratio_b_x96, sqrt_ratio_a_x96
    if sqrt_ratio_a_x96 == 0:
        raise ValueError("sqrtRatioA cannot be zero")

    numerator1 = liquidity << 96
    numerator2 = sqrt_ratio_b_x96 - sqrt_ratio_a_x96

    if round_up:
        # UnsafeMath.divRoundingUp(mulDivRoundingUp(...), sqrtRatioAX96)
        inner = mul_div_rounding_up(numerator1, numerator2, sqrt_ratio_b_x96)
        return inner // sqrt_ratio_a_x96 + (0 if inner % sqrt_ratio_a_x96 == 0 else 1)
    return mul_div(numerator1, numerator2, sqrt_ratio_b_x96) // sqrt_ratio_a_x96


def get_amount1_delta(
    sqrt_ratio_a_x96: int,
    sqrt_ratio_b_x96: int,
    liquidity: int,
    round_up: bool,
) -> int:
    """|Δtoken1| for a liquidity position between two prices (Solidity: getAmount1Delta)."""
    if sqrt_ratio_a_x96 > sqrt_ratio_b_x96:
        sqrt_ratio_a_x96, sqrt_ratio_b_x96 = sqrt_ratio_b_x96, sqrt_ratio_a_x96

    if round_up:
        return mul_div_rounding_up(liquidity, sqrt_ratio_b_x96 - sqrt_ratio_a_x96, Q96)
    return mul_div(liquidity, sqrt_ratio_b_x96 - sqrt_ratio_a_x96, Q96)


def get_amount0_delta_signed(sqrt_a: int, sqrt_b: int, liquidity: int) -> int:
    if liquidity < 0:
        return -get_amount0_delta(sqrt_a, sqrt_b, -liquidity, False)
    return get_amount0_delta(sqrt_a, sqrt_b, liquidity, True)


def get_amount1_delta_signed(sqrt_a: int, sqrt_b: int, liquidity: int) -> int:
    if liquidity < 0:
        return -get_amount1_delta(sqrt_a, sqrt_b, -liquidity, False)
    return get_amount1_delta(sqrt_a, sqrt_b, liquidity, True)


def get_next_sqrt_price_from_amount0_rounding_up(
    sqrt_px96: int, liquidity: int, amount: int, add: bool
) -> int:
    if amount == 0:
        return sqrt_px96
    numerator1 = liquidity << 96

    if add:
        product = amount * sqrt_px96
        if product // amount == sqrt_px96:  # no uint256 overflow
            denominator = numerator1 + product
            if denominator >= numerator1:
                return _require_uint160(mul_div_rounding_up(numerator1, sqrt_px96, denominator))
        return _require_uint160(-(-numerator1 // (numerator1 // sqrt_px96 + amount)))  # ceil

    product = amount * sqrt_px96
    if not (product // amount == sqrt_px96 and numerator1 > product):
        raise ValueError("price overflow on output subtraction")
    denominator = numerator1 - product
    return _require_uint160(mul_div_rounding_up(numerator1, sqrt_px96, denominator))


def get_next_sqrt_price_from_amount1_rounding_down(
    sqrt_px96: int, liquidity: int, amount: int, add: bool
) -> int:
    if add:
        quotient = (
            (amount << 96) // liquidity
            if amount <= _UINT160_MAX
            else mul_div(amount, Q96, liquidity)
        )
        return _require_uint160(sqrt_px96 + quotient)

    quotient = (
        (-(-(amount << 96) // liquidity))  # divRoundingUp
        if amount <= _UINT160_MAX
        else mul_div_rounding_up(amount, Q96, liquidity)
    )
    if sqrt_px96 <= quotient:
        raise ValueError("price underflow")
    return _require_uint160(sqrt_px96 - quotient)


def get_next_sqrt_price_from_input(
    sqrt_px96: int, liquidity: int, amount_in: int, zero_for_one: bool
) -> int:
    if sqrt_px96 <= 0 or liquidity <= 0:
        raise ValueError("price/liquidity must be positive")
    if zero_for_one:
        return get_next_sqrt_price_from_amount0_rounding_up(sqrt_px96, liquidity, amount_in, True)
    return get_next_sqrt_price_from_amount1_rounding_down(sqrt_px96, liquidity, amount_in, True)


def get_next_sqrt_price_from_output(
    sqrt_px96: int, liquidity: int, amount_out: int, zero_for_one: bool
) -> int:
    if sqrt_px96 <= 0 or liquidity <= 0:
        raise ValueError("price/liquidity must be positive")
    if zero_for_one:
        return get_next_sqrt_price_from_amount1_rounding_down(sqrt_px96, liquidity, amount_out, False)
    return get_next_sqrt_price_from_amount0_rounding_up(sqrt_px96, liquidity, amount_out, False)
