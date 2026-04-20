"""Event-replay (Path B) reconstruction from synthetic mint/burn events."""

from __future__ import annotations

from decimal import Decimal
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq

from fabdl.extract.liquidity_snapshots import snapshot_path_b
from fabdl.io.parquet import mint_burn_events_schema


def _write_events(path: Path, events: list[dict]) -> None:
    schema = mint_burn_events_schema()
    rows = [
        {
            "block_number": ev["block"],
            "timestamp": 0,
            "tx_hash": bytes(32),
            "log_index": 0,
            "event_type": ev["type"],
            "owner": bytes(20),
            "tick_lower": ev["lower"],
            "tick_upper": ev["upper"],
            "liquidity_delta": Decimal(ev["delta"]),
            "amount0_raw": Decimal(0),
            "amount1_raw": Decimal(0),
            "date": "2025-10-01",
        }
        for ev in events
    ]
    path.parent.mkdir(parents=True, exist_ok=True)
    pq.write_table(pa.Table.from_pylist(rows, schema=schema), path)


def test_single_mint_creates_two_ticks(tmp_path: Path):
    p = tmp_path / "mb.parquet"
    _write_events(p, [{"block": 100, "type": "mint", "lower": -10, "upper": 10, "delta": 1000}])
    out = snapshot_path_b(p, snapshot_block=200)
    by_tick = {d.tick: d for d in out}
    assert by_tick[-10].liquidity_net == 1000
    assert by_tick[10].liquidity_net == -1000
    assert by_tick[-10].liquidity_gross == 1000
    assert by_tick[10].liquidity_gross == 1000


def test_burn_offsets_mint(tmp_path: Path):
    p = tmp_path / "mb.parquet"
    _write_events(
        p,
        [
            {"block": 100, "type": "mint", "lower": -10, "upper": 10, "delta": 1000},
            {"block": 150, "type": "burn", "lower": -10, "upper": 10, "delta": -300},
        ],
    )
    out = snapshot_path_b(p, snapshot_block=200)
    by_tick = {d.tick: d for d in out}
    assert by_tick[-10].liquidity_net == 700
    assert by_tick[10].liquidity_net == -700


def test_snapshot_block_truncates_future_events(tmp_path: Path):
    p = tmp_path / "mb.parquet"
    _write_events(
        p,
        [
            {"block": 100, "type": "mint", "lower": -10, "upper": 10, "delta": 1000},
            {"block": 300, "type": "mint", "lower": -20, "upper": 20, "delta": 500},
        ],
    )
    out = snapshot_path_b(p, snapshot_block=200)
    # Second mint at block 300 should be excluded.
    ticks = {d.tick for d in out}
    assert ticks == {-10, 10}


def test_overlapping_positions_sum(tmp_path: Path):
    p = tmp_path / "mb.parquet"
    _write_events(
        p,
        [
            {"block": 100, "type": "mint", "lower": -10, "upper": 10, "delta": 1000},
            {"block": 110, "type": "mint", "lower": -10, "upper": 20, "delta": 500},
        ],
    )
    out = snapshot_path_b(p, snapshot_block=200)
    by_tick = {d.tick: d for d in out}
    assert by_tick[-10].liquidity_net == 1500
    assert by_tick[-10].liquidity_gross == 1500
    assert by_tick[10].liquidity_net == -1000
    assert by_tick[20].liquidity_net == -500
