"""Parquet schemas and append-then-compact helpers for Module 1 deliverables.

Large uint256 cumulatives are stored as ``decimal128(38,0)`` — enough headroom
for realistic amounts and cheap to read with pyarrow/polars. ``tx_hash`` is
stored as a 32-byte fixed-size-binary for density. Partition columns are
string-typed dates.
"""

from __future__ import annotations

from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq

DEC = pa.decimal128(38, 0)
DEC256 = pa.decimal256(76, 0)   # for raw uint256/uint128 fee-growth accumulators


def swap_events_schema() -> pa.Schema:
    return pa.schema(
        [
            ("block_number", pa.int64()),
            ("timestamp", pa.int64()),
            ("tx_hash", pa.binary(32)),
            ("log_index", pa.int32()),
            ("sender", pa.binary(20)),
            ("recipient", pa.binary(20)),
            ("amount0_raw", DEC),            # signed int256 as decimal
            ("amount1_raw", DEC),
            ("amount0_usdc", pa.float64()),   # human units
            ("amount1_weth", pa.float64()),
            ("sqrt_price_x96", DEC),
            ("liquidity", DEC),
            ("tick", pa.int32()),
            ("price_weth_per_usdc", pa.float64()),
            ("direction", pa.string()),       # 'zeroForOne' or 'oneForZero'
            ("usd_notional", pa.float64()),
            ("date", pa.string()),
        ]
    )


def mint_burn_events_schema() -> pa.Schema:
    return pa.schema(
        [
            ("block_number", pa.int64()),
            ("timestamp", pa.int64()),
            ("tx_hash", pa.binary(32)),
            ("log_index", pa.int32()),
            ("event_type", pa.string()),      # 'mint' | 'burn'
            ("owner", pa.binary(20)),
            ("tick_lower", pa.int32()),
            ("tick_upper", pa.int32()),
            ("liquidity_delta", DEC256),
            ("amount0_raw", DEC256),
            ("amount1_raw", DEC256),
            ("date", pa.string()),
        ]
    )


def slot0_schema() -> pa.Schema:
    return pa.schema(
        [
            ("date", pa.string()),
            ("snapshot_block", pa.int64()),
            ("timestamp", pa.int64()),
            ("sqrt_price_x96", DEC),
            ("price_weth_per_usdc", pa.float64()),
            ("tick", pa.int32()),
            ("observation_index", pa.int32()),
            ("observation_cardinality", pa.int32()),
            ("fee_protocol", pa.int32()),
            ("unlocked", pa.bool_()),
            ("fee_growth_global_0_x128", DEC256),
            ("fee_growth_global_1_x128", DEC256),
        ]
    )


def lp_analytics_schema() -> pa.Schema:
    return pa.schema(
        [
            ("date", pa.string()),
            ("position_id", pa.string()),          # "<tick_lower>_<tick_upper>"
            ("tick_lower", pa.int32()),
            ("tick_upper", pa.int32()),
            ("liquidity", DEC),                    # uint128 position liquidity
            ("entry_price", pa.float64()),         # USDC per WETH at entry
            ("price_usdc_per_weth", pa.float64()),
            ("amount0_usdc", pa.float64()),        # token0 holdings (human)
            ("amount1_weth", pa.float64()),        # token1 holdings (human)
            ("position_value_usd", pa.float64()),
            ("fees_token0_usdc", pa.float64()),    # cumulative from entry
            ("fees_token1_weth", pa.float64()),
            ("fees_usd", pa.float64()),
            ("il_ratio", pa.float64()),
            ("hodl_value_usd", pa.float64()),
            ("pnl_vs_hodl_usd", pa.float64()),    # fees + position_value - hodl_value
        ]
    )


def liquidity_snapshot_schema() -> pa.Schema:
    return pa.schema(
        [
            ("date", pa.string()),
            ("snapshot_block", pa.int64()),
            ("timestamp", pa.int64()),
            ("tick", pa.int32()),
            ("liquidity_net", DEC256),
            ("liquidity_gross", DEC256),
            ("fee_growth_outside_0_x128", DEC256),
            ("fee_growth_outside_1_x128", DEC256),
        ]
    )


def append_rows(path: Path, rows: list[dict], schema: pa.Schema) -> None:
    """Append rows as a new part-file under ``path`` (a directory of parts)."""
    if not rows:
        return
    path.mkdir(parents=True, exist_ok=True)
    table = pa.Table.from_pylist(rows, schema=schema)
    existing = sorted(path.glob("part-*.parquet"))
    part_path = path / f"part-{len(existing):06d}.parquet"
    pq.write_table(table, part_path, compression="zstd")


def compact(part_dir: Path, output_file: Path, schema: pa.Schema) -> int:
    """Merge all ``part-*.parquet`` files under ``part_dir`` into ``output_file``."""
    parts = sorted(part_dir.glob("part-*.parquet"))
    if not parts:
        return 0
    output_file.parent.mkdir(parents=True, exist_ok=True)
    tables = [pq.read_table(p, schema=schema) for p in parts]
    merged = pa.concat_tables(tables)
    pq.write_table(merged, output_file, compression="zstd")
    return merged.num_rows
