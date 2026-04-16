"""Pure-integer fixed-point helpers.

Ports of Uniswap v3-core FullMath.sol. Python's arbitrary-precision int makes the
Chinese-remainder-theorem gymnastics unnecessary: we just floor-divide and guard
the uint256 bound explicitly.

No floats. No numpy. Everything here is ``int``.
"""

UINT256_MAX = (1 << 256) - 1


def _require_uint256(x: int) -> int:
    if x < 0 or x > UINT256_MAX:
        raise OverflowError("result does not fit in uint256")
    return x


def mul_div(a: int, b: int, denominator: int) -> int:
    """floor(a * b / denominator), matching FullMath.mulDiv.

    Reverts (OverflowError) if denominator == 0 or the result exceeds uint256.
    """
    if denominator == 0:
        raise ZeroDivisionError("mul_div denominator is zero")
    return _require_uint256((a * b) // denominator)


def mul_div_rounding_up(a: int, b: int, denominator: int) -> int:
    """ceil(a * b / denominator), matching FullMath.mulDivRoundingUp."""
    result = mul_div(a, b, denominator)
    if (a * b) % denominator != 0:
        if result == UINT256_MAX:
            raise OverflowError("mul_div_rounding_up overflow")
        result += 1
    return result
