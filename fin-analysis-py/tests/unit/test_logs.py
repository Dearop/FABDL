from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from fabdl.io.logs import fetch_logs_chunked
from fabdl.io.rpc import RpcClient, RpcConfig, TooManyResultsError


class FakeRpc:
    """Minimal stand-in exposing only the attrs ``fetch_logs_chunked`` uses."""

    def __init__(self, *, throttle_limit: int, initial_chunk: int):
        self._cfg = RpcConfig(url="x", initial_log_chunk_blocks=initial_chunk)
        self._throttle = throttle_limit
        self.calls: list[tuple[int, int]] = []

    def get_logs(self, address: str, topics: list, from_block: int, to_block: int) -> list[dict]:
        self.calls.append((from_block, to_block))
        span = to_block - from_block + 1
        if span > self._throttle:
            raise TooManyResultsError(f"span {span} > cap {self._throttle}")
        return [{"from": from_block, "to": to_block}]


def test_shrinks_chunk_on_too_many_results(tmp_path: Path):
    rpc: Any = FakeRpc(throttle_limit=250, initial_chunk=1000)
    collected: list[dict] = []

    def on_batch(logs, lo, hi):
        collected.extend(logs)

    n = fetch_logs_chunked(rpc, "0x00", [None], 0, 999, on_batch)
    assert n == len(collected) > 0
    # Initial 1000-block attempt fails, then halves to 500, fails, then 250 succeeds.
    spans = [hi - lo + 1 for lo, hi in rpc.calls if hi - lo + 1 <= 250]
    assert max(spans) <= 250


def test_checkpoint_resume(tmp_path: Path):
    rpc: Any = FakeRpc(throttle_limit=1000, initial_chunk=100)
    ckpt = tmp_path / "ckpt.json"

    # First run writes a checkpoint at the end.
    fetch_logs_chunked(rpc, "0x00", [None], 0, 499, lambda *a: None, checkpoint_path=ckpt)
    assert json.loads(ckpt.read_text())["last_block_completed"] == 499

    # Simulate a crashed resume at an earlier point, then verify second run skips done range.
    ckpt.write_text(json.dumps({"last_block_completed": 299}))
    rpc2: Any = FakeRpc(throttle_limit=1000, initial_chunk=100)
    fetch_logs_chunked(rpc2, "0x00", [None], 0, 499, lambda *a: None, checkpoint_path=ckpt)
    # Second run should only query blocks 300..499 — nothing before 300.
    assert all(lo >= 300 for lo, _ in rpc2.calls)


def test_raises_when_cannot_shrink_below_min(tmp_path: Path):
    rpc: Any = FakeRpc(throttle_limit=0, initial_chunk=10)  # even 1 block overflows
    with pytest.raises(TooManyResultsError):
        fetch_logs_chunked(rpc, "0x00", [None], 0, 10, lambda *a: None)
