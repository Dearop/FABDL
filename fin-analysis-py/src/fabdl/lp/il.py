"""Impermanent loss and break-even analysis for a single V3 position.

Price convention: ``P = token1_per_token0`` in raw units, i.e. the native V3
price. For USDC/WETH (token0=USDC 6dec, token1=WETH 18dec) the human price
``USDC per WETH`` is ``(10**12) / P``. We stay in raw-price space internally;
callers convert at the boundary.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Callable

from fabdl.core.constants import Q96, TOKEN0_DECIMALS, TOKEN1_DECIMALS
from fabdl.core.tickmath import get_sqrt_ratio_at_tick
from fabdl.lp.position import amounts_from_liquidity


def _sqrt_x96_from_price(price_token1_per_token0: float) -> int:
    """``sqrtPriceX96`` from a float price in raw units. For analytics only."""
    if price_token1_per_token0 <= 0:
        raise ValueError("price must be positive")
    return int(math.sqrt(price_token1_per_token0) * Q96)


def _human_price_to_raw(human_usdc_per_weth: float) -> float:
    """``USDC per WETH`` (human) → raw ``P = reserve1_raw / reserve0_raw``.

    Raw = 10**(d1-d0) / human: 1 WETH wei maps to ``10**-12`` USDC wei at
    $1/WETH, so at $h/WETH the ratio token1/token0 is ``10**12/h``.
    """
    if human_usdc_per_weth <= 0:
        raise ValueError("price must be positive")
    return (10 ** (TOKEN1_DECIMALS - TOKEN0_DECIMALS)) / human_usdc_per_weth


@dataclass(frozen=True)
class ILPoint:
    price_usdc_per_weth: float
    amount0_human: float
    amount1_human: float
    position_value_usd: float
    hodl_value_usd: float
    il_ratio: float


def _position_value_usd_at_price(
    liquidity: int, tick_lower: int, tick_upper: int, price_usdc_per_weth: float
) -> tuple[float, float, float]:
    raw = _human_price_to_raw(price_usdc_per_weth)
    sqrt_x96 = _sqrt_x96_from_price(raw)
    a0, a1 = amounts_from_liquidity(
        liquidity,
        get_sqrt_ratio_at_tick(tick_lower),
        get_sqrt_ratio_at_tick(tick_upper),
        sqrt_x96,
    )
    a0_h = a0 / (10**TOKEN0_DECIMALS)
    a1_h = a1 / (10**TOKEN1_DECIMALS)
    value = a0_h + a1_h * price_usdc_per_weth
    return value, a0_h, a1_h


def il_curve(
    liquidity: int,
    tick_lower: int,
    tick_upper: int,
    entry_price_usdc_per_weth: float,
    price_grid_usdc_per_weth: list[float],
) -> list[ILPoint]:
    """IL ratio = (V(P) - HODL(P)) / HODL(P), HODL = holding entry amounts."""
    entry_value, a0_entry, a1_entry = _position_value_usd_at_price(
        liquidity, tick_lower, tick_upper, entry_price_usdc_per_weth
    )
    out: list[ILPoint] = []
    for price in price_grid_usdc_per_weth:
        value, a0, a1 = _position_value_usd_at_price(
            liquidity, tick_lower, tick_upper, price
        )
        hodl = a0_entry + a1_entry * price
        il = (value - hodl) / hodl if hodl > 0 else 0.0
        out.append(
            ILPoint(
                price_usdc_per_weth=price,
                amount0_human=a0,
                amount1_human=a1,
                position_value_usd=value,
                hodl_value_usd=hodl,
                il_ratio=il,
            )
        )
    return out


def break_even_prices(
    liquidity: int,
    tick_lower: int,
    tick_upper: int,
    entry_price_usdc_per_weth: float,
    fees_earned_usd: float,
    search_span: float = 0.5,
    *,
    max_iter: int = 100,
    tol: float = 1e-6,
) -> tuple[float | None, float | None]:
    """Return (lower, upper) prices where ``V(P) - HODL(P) + fees = 0``.

    Uses ``scipy.optimize.brentq`` on each side of the entry price. Search span
    is a fractional deviation (0.5 = ±50%). Returns ``None`` on that side if no
    sign change is found within the span.
    """
    from scipy.optimize import brentq

    entry_value, a0_e, a1_e = _position_value_usd_at_price(
        liquidity, tick_lower, tick_upper, entry_price_usdc_per_weth
    )

    def pnl(price: float) -> float:
        value, _, _ = _position_value_usd_at_price(
            liquidity, tick_lower, tick_upper, price
        )
        hodl = a0_e + a1_e * price
        return (value - hodl) + fees_earned_usd

    f0 = pnl(entry_price_usdc_per_weth)

    def _find(side: int) -> float | None:
        p_edge = entry_price_usdc_per_weth * (1 + side * search_span)
        try:
            f_edge = pnl(p_edge)
        except ValueError:
            return None
        # Need a strict sign change. f0 == 0 (no fees) is a degenerate "root"
        # at the entry price — not a meaningful break-even.
        if f0 == 0 or f0 * f_edge >= 0:
            return None
        lo, hi = sorted([entry_price_usdc_per_weth, p_edge])
        return float(brentq(pnl, lo, hi, maxiter=max_iter, xtol=tol))

    return _find(-1), _find(+1)


def il_from_callable(
    position_value_fn: Callable[[float], float],
    hodl_value_fn: Callable[[float], float],
    prices: list[float],
) -> list[float]:
    """Generic IL ratio given user-supplied valuation fns. Useful for tests."""
    out: list[float] = []
    for p in prices:
        h = hodl_value_fn(p)
        out.append((position_value_fn(p) - h) / h if h > 0 else 0.0)
    return out
