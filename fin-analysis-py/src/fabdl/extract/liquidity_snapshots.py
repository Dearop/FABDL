"""Module 1 — Daily liquidity-map snapshots.

Two reconstruction paths that produce the same parquet schema:

- **Path A (archive):** scan ``tickBitmap`` words over the full range at the
  daily block, enumerate initialized ticks, then batch ``ticks(tick)`` via
  Multicall3 to get ``liquidityNet``, ``liquidityGross``, and both
  ``feeGrowthOutside_*_X128`` values.
- **Path B (event-replay):** walk Mint/Burn events up to the daily block and
  maintain a per-tick running sum of liquidityNet / liquidityGross. No fee-
  growth information is produced here — those fields are null in Path B rows.

Selection is automatic based on ``RpcClient.supports_archive``; callers can
override by calling ``snapshot_path_a`` / ``snapshot_path_b`` directly.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal
from pathlib import Path

import polars as pl
from eth_abi import decode as abi_decode
from eth_abi import encode as abi_encode
from eth_utils import keccak

from fabdl.core.constants import POOL_ADDRESS, TICK_SPACING
from fabdl.core.tickmath import MAX_TICK, MIN_TICK
from fabdl.io.block_index import BlockIndex
from fabdl.io.parquet import append_rows, compact, liquidity_snapshot_schema
from fabdl.io.rpc import Call, RpcClient

log = logging.getLogger(__name__)

_TICK_BITMAP_SELECTOR = keccak(text="tickBitmap(int16)")[:4]
_TICKS_SELECTOR = keccak(text="ticks(int24)")[:4]

# The full valid word-position range for this pool's tick spacing.
_MIN_WORD = (MIN_TICK // TICK_SPACING) >> 8
_MAX_WORD = (MAX_TICK // TICK_SPACING) >> 8

_MULTICALL_BATCH_BITMAP = 100
_MULTICALL_BATCH_TICKS = 100


@dataclass
class TickDetails:
    tick: int
    liquidity_gross: int
    liquidity_net: int
    fee_growth_outside_0_x128: int
    fee_growth_outside_1_x128: int


# ---------------------------------------------------------------- Path A: archive

def _encode_bitmap_call(word_pos: int) -> bytes:
    return _TICK_BITMAP_SELECTOR + abi_encode(["int16"], [word_pos])


def _encode_ticks_call(tick: int) -> bytes:
    return _TICKS_SELECTOR + abi_encode(["int24"], [tick])


def _decode_bitmap(raw: bytes) -> int:
    (word,) = abi_decode(["uint256"], raw)
    return word


def _decode_tick_details(raw: bytes, tick: int) -> TickDetails:
    (
        liquidity_gross,
        liquidity_net,
        fg0,
        fg1,
        _tick_cum_outside,
        _secs_per_liq_outside,
        _secs_outside,
        _initialized,
    ) = abi_decode(
        ["uint128", "int128", "uint256", "uint256", "int56", "uint160", "uint32", "bool"],
        raw,
    )
    return TickDetails(
        tick=tick,
        liquidity_gross=liquidity_gross,
        liquidity_net=liquidity_net,
        fee_growth_outside_0_x128=fg0,
        fee_growth_outside_1_x128=fg1,
    )


def _enumerate_initialized_ticks(rpc: RpcClient, block: int) -> list[int]:
    """Scan every word in the valid range; return the ticks with a set bit."""
    words = list(range(_MIN_WORD, _MAX_WORD + 1))
    ticks: list[int] = []
    for i in range(0, len(words), _MULTICALL_BATCH_BITMAP):
        batch = words[i : i + _MULTICALL_BATCH_BITMAP]
        calls = [Call(target=POOL_ADDRESS, data=_encode_bitmap_call(w)) for w in batch]
        results = rpc.multicall(calls, block=block)
        for word_pos, raw in zip(batch, results, strict=True):
            word = _decode_bitmap(raw)
            if word == 0:
                continue
            for bit in range(256):
                if word & (1 << bit):
                    compressed_tick = (word_pos << 8) | bit
                    # Handle two's complement for negative wordPos (int16 was encoded signed above,
                    # but compressed_tick derived from (word_pos << 8 | bit) is already correct
                    # because word_pos retains its sign in Python int).
                    ticks.append(compressed_tick * TICK_SPACING)
    return sorted(ticks)


def _fetch_tick_details(rpc: RpcClient, block: int, ticks: list[int]) -> list[TickDetails]:
    out: list[TickDetails] = []
    for i in range(0, len(ticks), _MULTICALL_BATCH_TICKS):
        batch = ticks[i : i + _MULTICALL_BATCH_TICKS]
        calls = [Call(target=POOL_ADDRESS, data=_encode_ticks_call(t)) for t in batch]
        results = rpc.multicall(calls, block=block)
        out.extend(_decode_tick_details(r, t) for t, r in zip(batch, results, strict=True))
    return out


def snapshot_path_a(rpc: RpcClient, block: int) -> list[TickDetails]:
    ticks = _enumerate_initialized_ticks(rpc, block)
    log.info("path-A: %d initialized ticks at block %d", len(ticks), block)
    return _fetch_tick_details(rpc, block, ticks)


# ---------------------------------------------------------------- Path B: event-replay

def snapshot_path_b(mint_burn_parquet: Path, snapshot_block: int) -> list[TickDetails]:
    """Replay the ``mint_burn_events`` parquet up to ``snapshot_block`` inclusive."""
    df = pl.read_parquet(mint_burn_parquet).filter(pl.col("block_number") <= snapshot_block)
    positions: dict[int, dict[str, int]] = {}
    for row in df.iter_rows(named=True):
        delta = int(row["liquidity_delta"])
        lower, upper = row["tick_lower"], row["tick_upper"]
        for t, net_sign in ((lower, +1), (upper, -1)):
            bucket = positions.setdefault(t, {"net": 0, "gross": 0})
            bucket["net"] += net_sign * delta
            bucket["gross"] += abs(delta) if row["event_type"] == "mint" else -abs(delta)
    return [
        TickDetails(
            tick=t,
            liquidity_gross=b["gross"],
            liquidity_net=b["net"],
            fee_growth_outside_0_x128=0,
            fee_growth_outside_1_x128=0,
        )
        for t, b in sorted(positions.items())
        if b["gross"] != 0 or b["net"] != 0
    ]


# ---------------------------------------------------------------- snapshot writer

def _end_of_day_ts(day: date) -> int:
    return int(datetime.combine(day, datetime.max.time(), tzinfo=UTC).timestamp())


def extract_liquidity_snapshots_daily(
    rpc: RpcClient,
    start: date,
    end: date,
    out_dir: Path,
    *,
    force_path_b: bool = False,
    mint_burn_parquet: Path | None = None,
) -> int:
    block_index = BlockIndex(rpc, cache_path=out_dir / "checkpoints" / "block_index.sqlite")
    parts_dir = out_dir / "raw" / "liquidity_snapshots_parts"

    use_path_b = force_path_b or not rpc.supports_archive
    if use_path_b:
        if mint_burn_parquet is None:
            mint_burn_parquet = out_dir / "processed" / "mint_burn_events.parquet"
        if not mint_burn_parquet.exists():
            raise FileNotFoundError(
                f"Path B requires {mint_burn_parquet} — run `fabdl extract mints-burns` first"
            )

    day = start
    total_rows = 0
    while day <= end:
        block = block_index.block_at_timestamp(_end_of_day_ts(day))
        ts = block_index._ts_of(block)  # noqa: SLF001
        if use_path_b:
            details = snapshot_path_b(mint_burn_parquet, block)  # type: ignore[arg-type]
        else:
            details = snapshot_path_a(rpc, block)

        rows = [
            {
                "date": day.isoformat(),
                "snapshot_block": block,
                "timestamp": ts,
                "tick": d.tick,
                "liquidity_net": Decimal(d.liquidity_net),
                "liquidity_gross": Decimal(d.liquidity_gross),
                "fee_growth_outside_0_x128": Decimal(d.fee_growth_outside_0_x128),
                "fee_growth_outside_1_x128": Decimal(d.fee_growth_outside_1_x128),
            }
            for d in details
        ]
        append_rows(parts_dir, rows, liquidity_snapshot_schema())
        total_rows += len(rows)
        log.info("liquidity %s @ block %d: %d ticks", day.isoformat(), block, len(rows))
        day += timedelta(days=1)
    return total_rows


def compact_liquidity_snapshots(out_dir: Path) -> int:
    return compact(
        out_dir / "raw" / "liquidity_snapshots_parts",
        out_dir / "processed" / "liquidity_snapshots.parquet",
        liquidity_snapshot_schema(),
    )
