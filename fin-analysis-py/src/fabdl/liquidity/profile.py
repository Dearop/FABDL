"""Module 2 — Liquidity profile and TVL decomposition.

Given a daily ``liquidity_snapshots.parquet`` row set, compute:

- ``tick_ranges``: consecutive (tick_lower, tick_upper, active_liquidity) tuples
  between initialized ticks. Active liquidity is the running sum of
  ``liquidityNet`` up to but not including ``tick_upper``.
- ``tvl_decomposition``: the token0 / token1 amount held in each range at a
  given ``sqrtPriceX96``, plus a human-unit USD value.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date

import polars as pl

from fabdl.core.constants import TOKEN0_DECIMALS, TOKEN1_DECIMALS
from fabdl.core.sqrtmath import get_amount0_delta, get_amount1_delta
from fabdl.core.tickmath import get_sqrt_ratio_at_tick


@dataclass(frozen=True)
class TickRange:
    tick_lower: int
    tick_upper: int
    active_liquidity: int


def load_snapshot(
    parquet_path: str | bytes, snapshot_date: date | str | None = None
) -> pl.DataFrame:
    """Load a single date from ``liquidity_snapshots.parquet`` sorted by tick."""
    df = pl.read_parquet(parquet_path)
    if snapshot_date is not None:
        d = snapshot_date.isoformat() if isinstance(snapshot_date, date) else snapshot_date
        df = df.filter(pl.col("date") == d)
    if df.is_empty():
        raise ValueError(f"no snapshot rows for {snapshot_date!r}")
    return df.sort("tick")


def tick_ranges(snapshot: pl.DataFrame) -> list[TickRange]:
    """Consecutive ranges between initialized ticks with running active liquidity."""
    ticks = snapshot["tick"].to_list()
    nets = [int(x) for x in snapshot["liquidity_net"].to_list()]
    ranges: list[TickRange] = []
    active = 0
    for i, t in enumerate(ticks):
        active += nets[i]
        if i + 1 >= len(ticks):
            break
        if active != 0:
            ranges.append(TickRange(tick_lower=t, tick_upper=ticks[i + 1], active_liquidity=active))
    return ranges


@dataclass(frozen=True)
class RangeTVL:
    tick_lower: int
    tick_upper: int
    active_liquidity: int
    amount0_raw: int
    amount1_raw: int
    amount0_human: float
    amount1_human: float
    usd_value: float


def tvl_decomposition(
    snapshot: pl.DataFrame, sqrt_price_x96: int, weth_price_usd: float
) -> list[RangeTVL]:
    """Token0/token1 amounts per range at ``sqrt_price_x96``. USD uses ``weth_price_usd``.

    For USDC/WETH the USD value is ``amount0_human + amount1_human * weth_price_usd``.
    """
    decomp: list[RangeTVL] = []
    for rng in tick_ranges(snapshot):
        sqrt_a = get_sqrt_ratio_at_tick(rng.tick_lower)
        sqrt_b = get_sqrt_ratio_at_tick(rng.tick_upper)
        if sqrt_price_x96 <= sqrt_a:
            amount0 = get_amount0_delta(sqrt_a, sqrt_b, rng.active_liquidity, False)
            amount1 = 0
        elif sqrt_price_x96 >= sqrt_b:
            amount0 = 0
            amount1 = get_amount1_delta(sqrt_a, sqrt_b, rng.active_liquidity, False)
        else:
            amount0 = get_amount0_delta(sqrt_price_x96, sqrt_b, rng.active_liquidity, False)
            amount1 = get_amount1_delta(sqrt_a, sqrt_price_x96, rng.active_liquidity, False)
        a0_h = amount0 / (10**TOKEN0_DECIMALS)
        a1_h = amount1 / (10**TOKEN1_DECIMALS)
        decomp.append(
            RangeTVL(
                tick_lower=rng.tick_lower,
                tick_upper=rng.tick_upper,
                active_liquidity=rng.active_liquidity,
                amount0_raw=amount0,
                amount1_raw=amount1,
                amount0_human=a0_h,
                amount1_human=a1_h,
                usd_value=a0_h + a1_h * weth_price_usd,
            )
        )
    return decomp


def decomposition_to_polars(decomp: list[RangeTVL]) -> pl.DataFrame:
    return pl.DataFrame(
        {
            "tick_lower": [d.tick_lower for d in decomp],
            "tick_upper": [d.tick_upper for d in decomp],
            "active_liquidity": [str(d.active_liquidity) for d in decomp],
            "amount0_human": [d.amount0_human for d in decomp],
            "amount1_human": [d.amount1_human for d in decomp],
            "usd_value": [d.usd_value for d in decomp],
        }
    )


def total_tvl_usd(decomp: list[RangeTVL]) -> float:
    return sum(d.usd_value for d in decomp)
