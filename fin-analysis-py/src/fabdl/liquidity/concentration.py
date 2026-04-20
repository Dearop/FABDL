"""Module 2 — Concentration metrics on the liquidity distribution.

- **HHI (Herfindahl–Hirschman Index)** on TVL shares per initialized range.
  Returns a value in ``[0, 1]`` — higher = more concentrated.
- **ILR (Inverse Liquidity Ratio)** = ratio of virtual liquidity to the
  constant-product (V2-equivalent) liquidity at the same TVL. For a perfectly
  uniform V2 position ILR == 1; concentrated V3 positions can have ILR >> 1.
- **effective_tick_range**: the narrowest tick window that holds a given
  fraction of total TVL (e.g. 0.95).
"""

from __future__ import annotations

import math

from fabdl.liquidity.profile import RangeTVL


def hhi(decomp: list[RangeTVL]) -> float:
    total = sum(d.usd_value for d in decomp)
    if total <= 0:
        return 0.0
    return sum((d.usd_value / total) ** 2 for d in decomp)


def inverse_liquidity_ratio(
    decomp: list[RangeTVL], weth_price_usd: float, active_liquidity: int
) -> float:
    """ILR = L_virtual / L_v2_equivalent.

    The V2 equivalent at the same TVL satisfies ``x*y = k`` with the same USD
    value split 50/50 across both assets. Its "liquidity" in V3 units is
    ``sqrt(x * y)``.
    """
    total_usd = sum(d.usd_value for d in decomp)
    if total_usd <= 0 or weth_price_usd <= 0:
        return 0.0
    # Equivalent 50/50 V2 holdings in raw token units.
    x = (total_usd / 2)                                # USDC (already USD)
    y = (total_usd / 2) / weth_price_usd                # WETH
    l_v2 = math.sqrt(x * y)
    return active_liquidity / l_v2 if l_v2 > 0 else 0.0


def effective_tick_range(decomp: list[RangeTVL], fraction: float = 0.95) -> tuple[int, int] | None:
    """Narrowest contiguous window of ranges covering at least ``fraction`` of TVL.

    Ranges are assumed sorted by ``tick_lower`` (which is the output order of
    ``tick_ranges``). Returns ``(tick_low, tick_high)`` or None if TVL is zero.
    """
    total = sum(d.usd_value for d in decomp)
    if total <= 0 or not 0 < fraction <= 1:
        return None
    target = total * fraction
    n = len(decomp)
    best: tuple[int, int, int] | None = None  # (width_ticks, lower, upper)
    lo = 0
    running = 0.0
    for hi in range(n):
        running += decomp[hi].usd_value
        while running - decomp[lo].usd_value >= target and lo < hi:
            running -= decomp[lo].usd_value
            lo += 1
        if running >= target:
            width = decomp[hi].tick_upper - decomp[lo].tick_lower
            if best is None or width < best[0]:
                best = (width, decomp[lo].tick_lower, decomp[hi].tick_upper)
    return None if best is None else (best[1], best[2])
