"""Module 1 — Daily ``slot0()`` snapshots via ``eth_call`` at end-of-day blocks."""

from __future__ import annotations

import logging
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal
from pathlib import Path

from eth_abi import decode as abi_decode
from eth_utils import keccak

from fabdl.core.constants import POOL_ADDRESS, TOKEN0_DECIMALS, TOKEN1_DECIMALS
from fabdl.io.block_index import BlockIndex
from fabdl.io.parquet import append_rows, compact, slot0_schema
from fabdl.io.rpc import Call, RpcClient

log = logging.getLogger(__name__)

_SLOT0_SELECTOR = keccak(text="slot0()")[:4]
_FGG0_SELECTOR = keccak(text="feeGrowthGlobal0X128()")[:4]
_FGG1_SELECTOR = keccak(text="feeGrowthGlobal1X128()")[:4]
_Q96 = Decimal(2) ** 96
_USDC_SCALE = Decimal(10) ** TOKEN0_DECIMALS
_WETH_SCALE = Decimal(10) ** TOKEN1_DECIMALS


def _end_of_day_timestamps(start: date, end: date) -> list[tuple[date, int]]:
    """Return ``(date, end-of-day UTC timestamp)`` pairs inclusive."""
    out = []
    cur = start
    while cur <= end:
        eod = datetime.combine(cur, datetime.max.time(), tzinfo=UTC)
        out.append((cur, int(eod.timestamp())))
        cur += timedelta(days=1)
    return out


def _fetch_slot0(rpc: RpcClient, block: int) -> dict:
    results = rpc.multicall(
        [
            Call(POOL_ADDRESS, _SLOT0_SELECTOR),
            Call(POOL_ADDRESS, _FGG0_SELECTOR),
            Call(POOL_ADDRESS, _FGG1_SELECTOR),
        ],
        block=block,
    )
    (
        sqrt_price_x96,
        tick,
        obs_idx,
        obs_card,
        _obs_card_next,
        fee_protocol,
        unlocked,
    ) = abi_decode(["uint160", "int24", "uint16", "uint16", "uint16", "uint8", "bool"], results[0])
    (fgg0,) = abi_decode(["uint256"], results[1])
    (fgg1,) = abi_decode(["uint256"], results[2])
    return {
        "sqrt_price_x96": sqrt_price_x96,
        "tick": tick,
        "observation_index": obs_idx,
        "observation_cardinality": obs_card,
        "fee_protocol": fee_protocol,
        "unlocked": unlocked,
        "fee_growth_global_0_x128": fgg0,
        "fee_growth_global_1_x128": fgg1,
    }


def _price_weth_per_usdc(sqrt_price_x96: int) -> float:
    price_raw = (Decimal(sqrt_price_x96) / _Q96) ** 2
    return float(price_raw * (_USDC_SCALE / _WETH_SCALE))


def extract_slot0_daily(
    rpc: RpcClient,
    start: date,
    end: date,
    out_dir: Path,
) -> int:
    block_index = BlockIndex(rpc, cache_path=out_dir / "checkpoints" / "block_index.sqlite")
    parts_dir = out_dir / "raw" / "slot0_snapshots_parts"
    rows: list[dict] = []
    for day, eod_ts in _end_of_day_timestamps(start, end):
        block = block_index.block_at_timestamp(eod_ts)
        slot0 = _fetch_slot0(rpc, block)
        rows.append(
            {
                "date": day.isoformat(),
                "snapshot_block": block,
                "timestamp": block_index._ts_of(block),  # noqa: SLF001
                "sqrt_price_x96": Decimal(slot0["sqrt_price_x96"]),
                "price_weth_per_usdc": _price_weth_per_usdc(slot0["sqrt_price_x96"]),
                "tick": slot0["tick"],
                "observation_index": slot0["observation_index"],
                "observation_cardinality": slot0["observation_cardinality"],
                "fee_protocol": slot0["fee_protocol"],
                "unlocked": slot0["unlocked"],
                "fee_growth_global_0_x128": Decimal(slot0["fee_growth_global_0_x128"]),
                "fee_growth_global_1_x128": Decimal(slot0["fee_growth_global_1_x128"]),
            }
        )
        log.info("slot0 %s @ block %d: tick=%d", day.isoformat(), block, slot0["tick"])
    append_rows(parts_dir, rows, slot0_schema())
    return len(rows)


def compact_slot0(out_dir: Path) -> int:
    return compact(
        out_dir / "raw" / "slot0_snapshots_parts",
        out_dir / "processed" / "slot0_snapshots.parquet",
        slot0_schema(),
    )
