"""Chunked ``eth_getLogs`` with adaptive range sizing and checkpointed progress.

If a chunk overflows the endpoint's result cap (``TooManyResultsError``), the
range is halved and retried. After each successful chunk the checkpoint file is
updated so a crashed backfill resumes from exactly where it stopped.
"""

from __future__ import annotations

import json
import logging
from collections.abc import Callable, Iterable
from pathlib import Path

from fabdl.io.rpc import RpcClient, TooManyResultsError

log = logging.getLogger(__name__)

BatchCallback = Callable[[list[dict], int, int], None]
"""Called as ``on_batch(logs, from_block, to_block)`` after each successful chunk."""


def _read_checkpoint(path: Path) -> int | None:
    if not path.exists():
        return None
    return json.loads(path.read_text()).get("last_block_completed")


def _write_checkpoint(path: Path, last_block: int) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(".tmp")
    tmp.write_text(json.dumps({"last_block_completed": last_block}))
    tmp.replace(path)


def fetch_logs_chunked(
    rpc: RpcClient,
    address: str,
    topics: Iterable[str | None | list[str]],
    from_block: int,
    to_block: int,
    on_batch: BatchCallback,
    *,
    checkpoint_path: Path | None = None,
    initial_chunk: int | None = None,
    min_chunk: int = 1,
) -> int:
    """Fetch logs from ``from_block`` to ``to_block`` inclusive, batching by chunk.

    Returns the number of logs fetched. Resumes from the checkpoint if present.
    """
    topics_list = list(topics)
    chunk = initial_chunk or rpc._cfg.initial_log_chunk_blocks  # noqa: SLF001
    resume_from = _read_checkpoint(checkpoint_path) if checkpoint_path else None
    start = max(from_block, (resume_from + 1) if resume_from is not None else from_block)
    total = 0
    cursor = start
    while cursor <= to_block:
        window_end = min(cursor + chunk - 1, to_block)
        try:
            logs = rpc.get_logs(address, topics_list, cursor, window_end)
        except TooManyResultsError:
            if chunk <= min_chunk:
                raise
            chunk = max(min_chunk, chunk // 2)
            log.info("shrinking log chunk to %d blocks and retrying", chunk)
            continue
        on_batch(logs, cursor, window_end)
        total += len(logs)
        if checkpoint_path is not None:
            _write_checkpoint(checkpoint_path, window_end)
        cursor = window_end + 1
        # Grow chunk back up cautiously (doubled) after a clean success.
        if chunk < (initial_chunk or rpc._cfg.initial_log_chunk_blocks):  # noqa: SLF001
            chunk = min(chunk * 2, initial_chunk or rpc._cfg.initial_log_chunk_blocks)  # noqa: SLF001
    return total
