"""Module 4 — LP position analytics: fee tracking, IL, and PnL vs HODL.

Pipeline:
  1. Load ``slot0_snapshots.parquet`` (daily price + tick + feeGrowthGlobal).
  2. Load ``liquidity_snapshots.parquet`` (daily feeGrowthOutside per tick).
  3. For each day in the study window compute:
     - ``fee_growth_inside`` from global + outside values at tick_lower/upper.
     - Cumulative fees earned via ``fees_earned()``.
     - Position token amounts and USD value.
     - IL ratio vs HODL of entry amounts.
     - Combined PnL = position_value + cumulative_fees − HODL_value.
  4. Return a list of ``PositionSnapshot`` (one per calendar day).

Fee convention
--------------
``slot0_snapshots.parquet`` must contain ``fee_growth_global_0_x128`` and
``fee_growth_global_1_x128`` columns (added in Module 1 v2).  If either column
is absent the function raises ``MissingFeeGrowthError`` with instructions to
re-run ``fabdl extract slot0``.

Decimal128 handling
-------------------
PyArrow / polars stores uint256 cumulatives as ``Decimal128(38,0)``.  We
convert to Python ``int`` via ``_dec_to_int()`` which round-trips through
``str()`` — safe for the full 256-bit range.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from datetime import date
from decimal import Decimal
from pathlib import Path

import polars as pl

from fabdl.core.constants import Q96, TOKEN0_DECIMALS, TOKEN1_DECIMALS
from fabdl.core.tickmath import get_sqrt_ratio_at_tick
from fabdl.lp.fees import fee_apr, fee_growth_inside, fees_earned
from fabdl.lp.position import Position, amounts_from_liquidity


class MissingFeeGrowthError(RuntimeError):
    """Raised when slot0_snapshots lacks fee_growth_global columns."""


# ------------------------------------------------------------------ helpers


def _dec_to_int(val) -> int:
    """Convert Decimal128 / polars Decimal / Python int to Python int."""
    if val is None:
        return 0
    if isinstance(val, int):
        return val
    return int(str(val))


def _sqrt_x96_to_price_usdc_per_weth(sqrt_x96: int) -> float:
    """Human-readable USDC/WETH price from sqrtPriceX96."""
    price_raw = (Decimal(sqrt_x96) / Decimal(Q96)) ** 2  # weth_wei / usdc_wei
    # multiply by (10^18 / 10^6) to convert to WETH/USDC in human units, then invert
    price_weth_per_usdc = float(price_raw * Decimal(10 ** TOKEN0_DECIMALS) / Decimal(10 ** TOKEN1_DECIMALS))
    if price_weth_per_usdc <= 0:
        return 0.0
    return 1.0 / price_weth_per_usdc


# ------------------------------------------------------------------ data classes


@dataclass(frozen=True)
class PositionSnapshot:
    date: date
    price_usdc_per_weth: float
    amount0_usdc: float          # token0 (USDC) in position
    amount1_weth: float          # token1 (WETH) in position
    position_value_usd: float
    fees_token0_usdc: float      # cumulative from entry
    fees_token1_weth: float      # cumulative from entry
    fees_usd: float              # cumulative USD value of fees
    il_ratio: float              # (V - HODL) / HODL
    hodl_value_usd: float        # value of holding entry amounts at today's price
    pnl_vs_hodl_usd: float       # fees + position_value - hodl_value


@dataclass(frozen=True)
class PositionStats:
    entry_date: date
    exit_date: date
    tick_lower: int
    tick_upper: int
    liquidity: int
    entry_price: float
    exit_price: float
    entry_value_usd: float
    exit_value_usd: float
    total_fees_usd: float
    total_pnl_usd: float
    total_pnl_pct: float          # (pnl / entry_value) * 100
    hodl_pnl_usd: float           # hodl_value_exit - hodl_value_entry (= 0 by construction)
    il_ratio_final: float
    fee_apr: float
    days: int


# ------------------------------------------------------------------ inner logic


def _compute_snapshot(
    position: Position,
    day: date,
    tick_current: int,
    sqrt_price_x96: int,
    fee_growth_global: tuple[int, int],
    fgo_lower: tuple[int, int],
    fgo_upper: tuple[int, int],
    fgi_prev: tuple[int, int],
    cum_fees0: float,
    cum_fees1: float,
    entry_price: float,
    entry_amount0: float,
    entry_amount1: float,
) -> tuple[PositionSnapshot, tuple[int, int], float, float]:
    """Compute one day's snapshot.

    Returns ``(snapshot, new_fgi, new_cum_fees0, new_cum_fees1)``.
    """
    fgi_now = fee_growth_inside(
        tick_current,
        position.tick_lower,
        position.tick_upper,
        fgo_lower,
        fgo_upper,
        fee_growth_global,
    )

    raw0, raw1 = fees_earned(position.liquidity, fgi_now, fgi_prev)
    day_fees0 = raw0 / (10 ** TOKEN0_DECIMALS)
    day_fees1 = raw1 / (10 ** TOKEN1_DECIMALS)
    cum_fees0 += day_fees0
    cum_fees1 += day_fees1

    price = _sqrt_x96_to_price_usdc_per_weth(sqrt_price_x96)

    a0_raw, a1_raw = amounts_from_liquidity(
        position.liquidity,
        get_sqrt_ratio_at_tick(position.tick_lower),
        get_sqrt_ratio_at_tick(position.tick_upper),
        sqrt_price_x96,
    )
    a0 = a0_raw / (10 ** TOKEN0_DECIMALS)
    a1 = a1_raw / (10 ** TOKEN1_DECIMALS)
    pos_value = a0 + a1 * price

    fees_usd = cum_fees0 + cum_fees1 * price
    hodl = entry_amount0 + entry_amount1 * price
    il = (pos_value - hodl) / hodl if hodl > 0 else 0.0
    pnl = fees_usd + pos_value - hodl

    snap = PositionSnapshot(
        date=day,
        price_usdc_per_weth=price,
        amount0_usdc=a0,
        amount1_weth=a1,
        position_value_usd=pos_value,
        fees_token0_usdc=cum_fees0,
        fees_token1_weth=cum_fees1,
        fees_usd=fees_usd,
        il_ratio=il,
        hodl_value_usd=hodl,
        pnl_vs_hodl_usd=pnl,
    )
    return snap, fgi_now, cum_fees0, cum_fees1


# ------------------------------------------------------------------ public API


def track_position(
    position: Position,
    data_dir: Path,
    from_date: date,
    to_date: date,
) -> list[PositionSnapshot]:
    """Track a synthetic LP position over the study window.

    Loads ``slot0_snapshots.parquet`` and ``liquidity_snapshots.parquet`` from
    ``data_dir/processed/``.  The position's ``liquidity`` is treated as
    constant (no partial burns or fee collections).

    Parameters
    ----------
    position:
        Tick range and virtual liquidity.
    data_dir:
        Root data directory (the one containing ``processed/``).
    from_date / to_date:
        Inclusive date window.

    Returns
    -------
    list[PositionSnapshot]
        One snapshot per calendar day in the window, sorted ascending.
    """
    slot0_path = data_dir / "processed" / "slot0_snapshots.parquet"
    liq_path = data_dir / "processed" / "liquidity_snapshots.parquet"

    slot0_df = pl.read_parquet(slot0_path).filter(
        (pl.col("date") >= from_date.isoformat())
        & (pl.col("date") <= to_date.isoformat())
    ).sort("date")

    if "fee_growth_global_0_x128" not in slot0_df.columns:
        raise MissingFeeGrowthError(
            "slot0_snapshots.parquet is missing fee_growth_global columns. "
            "Re-run `fabdl extract slot0` to regenerate it with the updated extractor."
        )

    liq_df = pl.read_parquet(liq_path).filter(
        (pl.col("date") >= from_date.isoformat())
        & (pl.col("date") <= to_date.isoformat())
        & pl.col("tick").is_in([position.tick_lower, position.tick_upper])
    )

    # Index liquidity rows: date -> {tick -> (fgo0, fgo1)}
    liq_index: dict[str, dict[int, tuple[int, int]]] = {}
    for row in liq_df.iter_rows(named=True):
        d = row["date"]
        t = int(row["tick"])
        fgo0 = _dec_to_int(row["fee_growth_outside_0_x128"])
        fgo1 = _dec_to_int(row["fee_growth_outside_1_x128"])
        liq_index.setdefault(d, {})[t] = (fgo0, fgo1)

    snapshots: list[PositionSnapshot] = []
    fgi_prev: tuple[int, int] | None = None
    cum_fees0 = 0.0
    cum_fees1 = 0.0
    entry_price: float | None = None
    entry_amount0: float | None = None
    entry_amount1: float | None = None

    for row in slot0_df.iter_rows(named=True):
        d_str: str = row["date"]
        day = date.fromisoformat(d_str)
        tick_current = int(row["tick"])
        sqrt_x96 = _dec_to_int(row["sqrt_price_x96"])
        fgg = (
            _dec_to_int(row["fee_growth_global_0_x128"]),
            _dec_to_int(row["fee_growth_global_1_x128"]),
        )

        day_ticks = liq_index.get(d_str, {})
        fgo_lower = day_ticks.get(position.tick_lower, (0, 0))
        fgo_upper = day_ticks.get(position.tick_upper, (0, 0))

        fgi_today = fee_growth_inside(
            tick_current,
            position.tick_lower,
            position.tick_upper,
            fgo_lower,
            fgo_upper,
            fgg,
        )

        if fgi_prev is None:
            # Entry day: record state, no fee delta yet.
            fgi_prev = fgi_today
            price = _sqrt_x96_to_price_usdc_per_weth(sqrt_x96)
            entry_price = price
            a0_raw, a1_raw = amounts_from_liquidity(
                position.liquidity,
                get_sqrt_ratio_at_tick(position.tick_lower),
                get_sqrt_ratio_at_tick(position.tick_upper),
                sqrt_x96,
            )
            entry_amount0 = a0_raw / (10 ** TOKEN0_DECIMALS)
            entry_amount1 = a1_raw / (10 ** TOKEN1_DECIMALS)
            pos_value = entry_amount0 + entry_amount1 * price
            snap = PositionSnapshot(
                date=day,
                price_usdc_per_weth=price,
                amount0_usdc=entry_amount0,
                amount1_weth=entry_amount1,
                position_value_usd=pos_value,
                fees_token0_usdc=0.0,
                fees_token1_weth=0.0,
                fees_usd=0.0,
                il_ratio=0.0,
                hodl_value_usd=pos_value,
                pnl_vs_hodl_usd=0.0,
            )
            snapshots.append(snap)
            continue

        snap, fgi_prev, cum_fees0, cum_fees1 = _compute_snapshot(
            position=position,
            day=day,
            tick_current=tick_current,
            sqrt_price_x96=sqrt_x96,
            fee_growth_global=fgg,
            fgo_lower=fgo_lower,
            fgo_upper=fgo_upper,
            fgi_prev=fgi_prev,
            cum_fees0=cum_fees0,
            cum_fees1=cum_fees1,
            entry_price=entry_price,
            entry_amount0=entry_amount0,
            entry_amount1=entry_amount1,
        )
        snapshots.append(snap)

    return snapshots


def position_stats(snapshots: list[PositionSnapshot], position: Position) -> PositionStats:
    """Aggregate summary from a list of daily snapshots."""
    if not snapshots:
        raise ValueError("no snapshots to summarize")
    first = snapshots[0]
    last = snapshots[-1]
    window_seconds = max(1.0, (last.date - first.date).total_seconds())
    return PositionStats(
        entry_date=first.date,
        exit_date=last.date,
        tick_lower=position.tick_lower,
        tick_upper=position.tick_upper,
        liquidity=position.liquidity,
        entry_price=first.price_usdc_per_weth,
        exit_price=last.price_usdc_per_weth,
        entry_value_usd=first.position_value_usd,
        exit_value_usd=last.position_value_usd,
        total_fees_usd=last.fees_usd,
        total_pnl_usd=last.pnl_vs_hodl_usd,
        total_pnl_pct=(last.pnl_vs_hodl_usd / first.position_value_usd * 100.0)
        if first.position_value_usd > 0
        else 0.0,
        hodl_pnl_usd=last.hodl_value_usd - first.hodl_value_usd,
        il_ratio_final=last.il_ratio,
        fee_apr=fee_apr(last.fees_usd, first.position_value_usd, window_seconds),
        days=(last.date - first.date).days + 1,
    )


def snapshots_to_rows(
    snapshots: list[PositionSnapshot], position: Position
) -> list[dict]:
    """Convert snapshots to dicts suitable for ``append_rows()``."""
    from decimal import Decimal

    pid = f"{position.tick_lower}_{position.tick_upper}"
    rows = []
    for s in snapshots:
        rows.append(
            {
                "date": s.date.isoformat(),
                "position_id": pid,
                "tick_lower": position.tick_lower,
                "tick_upper": position.tick_upper,
                "liquidity": Decimal(position.liquidity),
                "entry_price": snapshots[0].price_usdc_per_weth,
                "price_usdc_per_weth": s.price_usdc_per_weth,
                "amount0_usdc": s.amount0_usdc,
                "amount1_weth": s.amount1_weth,
                "position_value_usd": s.position_value_usd,
                "fees_token0_usdc": s.fees_token0_usdc,
                "fees_token1_weth": s.fees_token1_weth,
                "fees_usd": s.fees_usd,
                "il_ratio": s.il_ratio,
                "hodl_value_usd": s.hodl_value_usd,
                "pnl_vs_hodl_usd": s.pnl_vs_hodl_usd,
            }
        )
    return rows
